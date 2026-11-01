// The codec-pages encoder for ALP and zstd: ../README.md defines the series
// and bodies files, the block rule, the round trip and the command line; the
// body layout of `alp` is in README.md beside this file.
//
//   codec-pages-alp <alp|zstd3|zstd19> <series file> <out dir> <L1,L2,...>
//   codec-pages-alp --verify <alp|zstd3|zstd19> <series file> <bodies file>
//
// The second form decodes a bodies file again in a fresh process and checks
// every block against the series file.
//
// ALP is the authors' library (cwida/ALP) used unmodified through its
// primitives: one block is one ALP row group (more than one when a block holds
// over 102,400 values), cut into vectors of 1,024 values, the last one shorter
// when the block is. zstd is libzstd's simple API on the block's raw
// little-endian float64 bytes.
//
// With CODEC_PAGES_TRACE=1 in the environment, `alp` also writes one JSON line
// a vector to stderr: its scheme and the parameters ALP chose for it.

#include "alp.hpp"
#include "zstd.h"

#include <algorithm>
#include <cerrno>
#include <cmath>
#include <cstdint>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <random>
#include <stdexcept>
#include <string>
#include <sys/types.h>
#include <vector>

#if !defined(__BYTE_ORDER__) || __BYTE_ORDER__ != __ORDER_LITTLE_ENDIAN__
#error "the harness's files are little-endian and so is this program's memory"
#endif

#ifndef ALP_COMMIT
#define ALP_COMMIT "unknown"
#endif

namespace {

constexpr size_t VEC = alp::config::VECTOR_SIZE;  // 1,024 values a vector
constexpr size_t RG  = alp::config::ROWGROUP_SIZE; // 102,400 values a row group

struct Error : std::runtime_error {
	using std::runtime_error::runtime_error;
};

// ---------------------------------------------------------------- bytes

void put_u8(std::vector<uint8_t>& b, uint8_t v) { b.push_back(v); }
void put_u16(std::vector<uint8_t>& b, uint16_t v) {
	b.push_back(uint8_t(v));
	b.push_back(uint8_t(v >> 8));
}
void put_u32(std::vector<uint8_t>& b, uint32_t v) {
	for (int i = 0; i < 4; i++) b.push_back(uint8_t(v >> (8 * i)));
}
void put_u64(std::vector<uint8_t>& b, uint64_t v) {
	for (int i = 0; i < 8; i++) b.push_back(uint8_t(v >> (8 * i)));
}

struct Reader {
	const uint8_t* p;
	size_t         n;
	size_t         at = 0;

	const uint8_t* take(size_t k) {
		if (k > n - at) throw Error("body ends early");
		const uint8_t* q = p + at;
		at += k;
		return q;
	}
	uint8_t  u8() { return *take(1); }
	uint16_t u16() {
		const uint8_t* q = take(2);
		return uint16_t(q[0] | (q[1] << 8));
	}
	uint32_t u32() {
		const uint8_t* q = take(4);
		uint32_t       v = 0;
		for (int i = 3; i >= 0; i--) v = (v << 8) | q[i];
		return v;
	}
	uint64_t u64() {
		const uint8_t* q = take(8);
		uint64_t       v = 0;
		for (int i = 7; i >= 0; i--) v = (v << 8) | q[i];
		return v;
	}
};

// ------------------------------------------------- packed data of a vector
//
// FastLanes packs a vector of 1,024 T-bit integers (T = 64 for ALP's integers
// and ALP_RD's right parts, 16 for ALP_RD's left parts) as LANES = 1024 / T
// lanes: value i belongs to lane i mod LANES and is row i / LANES of it; a
// lane's rows are written bw bits each, LSB first, into its words, and word w
// of lane l is out[l + LANES * w], so the buffer is 1,024 x bw / 8 bytes.
//
// A vector of m < 1,024 values leaves rows of padding at the end of every
// lane. pack_bits keeps only the bits of the m real values: words in buffer
// order, of each word its low bits that belong to rows below m, as one LSB-first
// bit stream of exactly m x bw bits in ceil(m x bw / 8) bytes. For m = 1,024 it
// keeps every bit and the bytes are FastLanes' buffer unchanged. unpack_bits
// puts the bits back where they were; the bits it cannot know (padding rows)
// get `fill`, 0 in decoding.

struct BitWriter {
	std::vector<uint8_t>& out;
	uint64_t              acc  = 0;
	unsigned              nacc = 0;

	void put(uint64_t v, unsigned k) {
		while (k > 0) {
			const unsigned space = 64 - nacc;
			const unsigned take  = k < space ? k : space;
			const uint64_t part  = take == 64 ? v : (v & ((uint64_t(1) << take) - 1));
			acc |= part << nacc;
			nacc += take;
			v = take == 64 ? 0 : v >> take;
			k -= take;
			if (nacc == 64) {
				put_u64(out, acc);
				acc  = 0;
				nacc = 0;
			}
		}
	}
	void flush() {
		while (nacc > 0) {
			out.push_back(uint8_t(acc));
			acc >>= 8;
			nacc = nacc > 8 ? nacc - 8 : 0;
		}
	}
};

struct BitReader {
	const uint8_t* p;
	size_t         nbits;
	size_t         at = 0;

	uint64_t get(unsigned k) {
		if (k > nbits - at) throw Error("packed bits end early");
		uint64_t v = 0;
		for (unsigned got = 0; got < k;) {
			const unsigned off  = unsigned(at & 7);
			const unsigned take = std::min(8 - off, k - got);
			const uint64_t bits = (uint64_t(p[at >> 3]) >> off) & ((uint64_t(1) << take) - 1);
			v |= bits << got;
			got += take;
			at += take;
		}
		return v;
	}
};

template <typename T>
unsigned lane_bits_in_word(size_t m, unsigned bw, unsigned lane, unsigned word) {
	constexpr unsigned TB    = sizeof(T) * 8;
	constexpr unsigned LANES = VEC / TB;
	const uint64_t     rows  = m > lane ? (m - lane + LANES - 1) / LANES : 0;
	const int64_t      k     = int64_t(rows * bw) - int64_t(word) * TB;
	return k <= 0 ? 0 : (k >= TB ? TB : unsigned(k));
}

size_t packed_bytes(size_t m, unsigned bw) { return (m * bw + 7) / 8; }

template <typename T>
void pack_bits(const T* packed, unsigned bw, size_t m, std::vector<uint8_t>& out) {
	constexpr unsigned LANES = VEC / (sizeof(T) * 8);
	const size_t       start = out.size();
	BitWriter          w {out};
	for (unsigned word = 0; word < bw; word++) {
		for (unsigned lane = 0; lane < LANES; lane++) {
			const unsigned k = lane_bits_in_word<T>(m, bw, lane, word);
			if (k) w.put(uint64_t(packed[lane + LANES * word]), k);
		}
	}
	w.flush();
	if (out.size() - start != packed_bytes(m, bw)) throw Error("pack_bits: wrong size");
}

template <typename T>
void unpack_bits(Reader& r, unsigned bw, size_t m, T* packed, T fill = 0) {
	constexpr unsigned TB    = sizeof(T) * 8;
	constexpr unsigned LANES = VEC / TB;
	const size_t       bytes = packed_bytes(m, bw);
	BitReader          br {r.take(bytes), m * bw};
	for (unsigned word = 0; word < bw; word++) {
		for (unsigned lane = 0; lane < LANES; lane++) {
			const unsigned k    = lane_bits_in_word<T>(m, bw, lane, word);
			const uint64_t mask = k == 64 ? ~uint64_t(0) : ((uint64_t(1) << k) - 1);
			const uint64_t bits = k ? br.get(k) : 0;
			packed[lane + LANES * word] = T((uint64_t(fill) & ~mask) | bits);
		}
	}
}

// pack_bits must drop only padding: pack random integers with FastLanes,
// keep m values' bits, restore the rest as ones, and every value below m must
// come back. Run once before any block is encoded.
template <typename T>
void self_test_pack_bits(const std::vector<unsigned>& widths) {
	std::mt19937_64 rnd(20261101);
	alignas(64) static T in[VEC], packed[VEC], back[VEC], out[VEC];
	const size_t         ms[] = {1, 2, 15, 16, 17, 31, 63, 64, 65, 100, 119, 120, 240, 241, 999, 1000, 1023, 1024};
	for (unsigned bw : widths) {
		for (size_t m : ms) {
			for (size_t i = 0; i < VEC; i++) {
				const uint64_t v = rnd();
				in[i]            = T(bw >= sizeof(T) * 8 ? v : (v & ((uint64_t(1) << bw) - 1)));
			}
			const T base = 0;
			std::memset(packed, 0, sizeof(packed));
			ffor::ffor(in, packed, uint8_t(bw), &base);
			std::vector<uint8_t> bytes;
			pack_bits(packed, bw, m, bytes);
			Reader r {bytes.data(), bytes.size()};
			unpack_bits(r, bw, m, back, T(~T(0)));
			if (r.at != bytes.size()) throw Error("self test: bytes left over");
			unffor::unffor(back, out, uint8_t(bw), &base);
			for (size_t i = 0; i < m; i++) {
				if (out[i] != in[i]) {
					throw Error("self test: pack_bits lost value " + std::to_string(i) + " at bw " +
					            std::to_string(bw) + ", m " + std::to_string(m));
				}
			}
			if (m == VEC && std::memcmp(bytes.data(), packed, bytes.size()) != 0) {
				throw Error("self test: a full vector's bytes are not FastLanes' buffer");
			}
		}
	}
}

// ------------------------------------------------------------------- ALP

struct AlpTotals {
	double   estimate_bits      = 0; // ALP's compression-ratio benchmark accounting
	uint64_t full_vector_extra  = 0; // bytes partial vectors would add as whole FastLanes buffers
	uint64_t alp_rd_rowgroups   = 0;
	uint64_t alp_rowgroups      = 0;
	uint64_t exceptions         = 0;
	uint64_t falp_mismatch      = 0; // blocks where fused falp disagrees with unffor + decode
};

bool trace_enabled() {
	const char* t = std::getenv("CODEC_PAGES_TRACE");
	return t && *t && std::strcmp(t, "0") != 0;
}

struct AlpBuffers {
	alignas(64) double   in[VEC];
	alignas(64) double   exc[VEC];
	alignas(64) uint16_t pos[VEC];
	alignas(64) int64_t  enc[VEC];
	alignas(64) int64_t  packed[VEC];
	alignas(64) uint64_t right[VEC];
	alignas(64) uint64_t right_packed[VEC];
	alignas(64) uint16_t left[VEC];
	alignas(64) uint16_t left_packed[VEC];
	alignas(64) uint16_t rd_exc[VEC];
	alignas(64) double   out[VEC];
	alignas(64) double   out_falp[VEC];
	std::vector<double>  sample = std::vector<double>(RG);
};

AlpBuffers& buffers() {
	static AlpBuffers* b = new AlpBuffers();
	return *b;
}

void alp_encode_block(const double* v, size_t n, std::vector<uint8_t>& body, AlpTotals& tot, size_t series,
                      size_t block) {
	AlpBuffers& b     = buffers();
	const bool  trace = trace_enabled();
	put_u32(body, uint32_t(n));
	for (size_t rg0 = 0, rg = 0; rg0 < n; rg0 += RG, rg++) {
		const size_t       rg_n = std::min(RG, n - rg0);
		const double*      rgv  = v + rg0;
		alp::state<double> stt;
		// First-level sampling of the row group picks the scheme and, for ALP,
		// the best k (exponent, factor) combinations.
		alp::encoder<double>::init(rgv, 0, rg_n, b.sample.data(), stt);
		put_u8(body, uint8_t(stt.scheme));

		if (stt.scheme == alp::Scheme::ALP_RD) {
			tot.alp_rd_rowgroups++;
			alp::rd_encoder<double>::init(rgv, 0, rg_n, b.sample.data(), stt);
			put_u8(body, stt.right_bit_width);
			put_u8(body, stt.left_bit_width);
			put_u8(body, stt.actual_dictionary_size);
			for (size_t i = 0; i < stt.actual_dictionary_size; i++) put_u16(body, stt.left_parts_dict[i]);
			for (size_t off = 0, vi = 0; off < rg_n; off += VEC, vi++) {
				const size_t m = std::min(VEC, rg_n - off);
				std::memcpy(b.in, rgv + off, m * sizeof(double));
				// ALP's example compressor fills an incomplete ALP_RD vector with its
				// first value; exceptions that fall in the padding are dropped.
				for (size_t i = m; i < VEC; i++) b.in[i] = b.in[0];
				stt.vector_size = uint16_t(m);
				uint16_t exc_c  = 0;
				alp::rd_encoder<double>::encode(b.in, b.rd_exc, b.pos, &exc_c, b.right, b.left, stt);
				uint16_t c = 0;
				while (c < exc_c && b.pos[c] < m) c++;
				const uint64_t zero64 = 0;
				const uint16_t zero16 = 0;
				ffor::ffor(b.right, b.right_packed, stt.right_bit_width, &zero64);
				ffor::ffor(b.left, b.left_packed, stt.left_bit_width, &zero16);
				put_u16(body, c);
				pack_bits(b.left_packed, stt.left_bit_width, m, body);
				pack_bits(b.right_packed, stt.right_bit_width, m, body);
				for (size_t i = 0; i < c; i++) put_u16(body, b.rd_exc[i]);
				for (size_t i = 0; i < c; i++) put_u16(body, b.pos[i]);

				tot.exceptions += c;
				tot.estimate_bits += double(m) * (stt.right_bit_width + stt.left_bit_width) +
				                     double(c) * (alp::RD_EXCEPTION_SIZE + alp::RD_EXCEPTION_POSITION_SIZE) +
				                     double(m) * (alp::config::MAX_RD_DICTIONARY_SIZE * 16) / double(RG);
				if (m < VEC) {
					tot.full_vector_extra += VEC * stt.left_bit_width / 8 - packed_bytes(m, stt.left_bit_width);
					tot.full_vector_extra += VEC * stt.right_bit_width / 8 - packed_bytes(m, stt.right_bit_width);
				}
				if (trace) {
					std::fprintf(stderr,
					             "{\"series\":%zu,\"block\":%zu,\"rowgroup\":%zu,\"vector\":%zu,\"scheme\":\"ALP_RD\","
					             "\"values\":%zu,\"right_bw\":%u,\"left_bw\":%u,\"dict\":%u,\"exceptions\":%u}\n",
					             series, block, rg, vi, m, unsigned(stt.right_bit_width),
					             unsigned(stt.left_bit_width), unsigned(stt.actual_dictionary_size), unsigned(c));
				}
			}
			continue;
		}

		tot.alp_rowgroups++;
		for (size_t off = 0, vi = 0; off < rg_n; off += VEC, vi++) {
			const size_t m = std::min(VEC, rg_n - off);
			std::memcpy(b.in, rgv + off, m * sizeof(double));
			stt.vector_size = uint16_t(m); // second-level sampling looks at the m real values only
			uint16_t exc_c  = 0;
			if (m < VEC) {
				// PRIMITIVES.md, "Last Vector Encoding": fill the vector with its
				// first non-exception value after encoding, then encode again.
				// The first pass, padded with 0.0, finds which real values are
				// exceptions; the padding then copies a real non-exception value,
				// so it adds no exception and does not widen the frame of reference.
				for (size_t i = m; i < VEC; i++) b.in[i] = 0.0;
				alp::encoder<double>::encode(b.in, b.exc, b.pos, &exc_c, b.enc, stt);
				double   filler = 0.0;
				uint16_t k      = 0;
				for (size_t i = 0; i < m; i++) {
					while (k < exc_c && b.pos[k] < i) k++;
					if (k < exc_c && b.pos[k] == i) continue;
					filler = b.in[i];
					break;
				}
				for (size_t i = m; i < VEC; i++) b.in[i] = filler;
			}
			alp::encoder<double>::encode(b.in, b.exc, b.pos, &exc_c, b.enc, stt);
			uint16_t c = 0;
			while (c < exc_c && b.pos[c] < m) c++;
			alp::bw_t bw   = 0;
			int64_t   base = 0;
			alp::encoder<double>::analyze_ffor(b.enc, bw, &base);
			ffor::ffor(b.enc, b.packed, bw, &base);

			put_u8(body, stt.exp);
			put_u8(body, stt.fac);
			put_u16(body, c);
			put_u64(body, uint64_t(base));
			put_u8(body, bw);
			pack_bits(reinterpret_cast<const uint64_t*>(b.packed), bw, m, body);
			for (size_t i = 0; i < c; i++) {
				uint64_t bits;
				std::memcpy(&bits, &b.exc[i], 8);
				put_u64(body, bits);
			}
			for (size_t i = 0; i < c; i++) put_u16(body, b.pos[i]);

			tot.exceptions += c;
			tot.estimate_bits += double(m) * bw +
			                     double(c) * (alp::Constants<double>::EXCEPTION_SIZE + alp::EXCEPTION_POSITION_SIZE) +
			                     (8 + 8 + 8 + 64);
			if (m < VEC) tot.full_vector_extra += VEC * bw / 8 - packed_bytes(m, bw);
			if (trace) {
				std::fprintf(stderr,
				             "{\"series\":%zu,\"block\":%zu,\"rowgroup\":%zu,\"vector\":%zu,\"scheme\":\"ALP\","
				             "\"values\":%zu,\"exp\":%u,\"fac\":%u,\"bw\":%u,\"exceptions\":%u,\"k\":%u}\n",
				             series, block, rg, vi, m, unsigned(stt.exp), unsigned(stt.fac), unsigned(bw), unsigned(c),
				             unsigned(stt.k_combinations));
			}
		}
	}
}

// Decodes an ALP body with ALP's own decoders. Returns false when the fused
// decoder (falp) disagreed with the unfused one anywhere; the unfused result
// (unffor + decode + patch_exceptions, as ALP's benchmarks decode) is `out`.
bool alp_decode_block(const uint8_t* p, size_t len, std::vector<double>& out) {
	AlpBuffers& b = buffers();
	Reader      r {p, len};
	const size_t n = r.u32();
	out.assign(n, 0.0);
	bool fused_agrees = true;
	for (size_t rg0 = 0; rg0 < n; rg0 += RG) {
		const size_t rg_n   = std::min(RG, n - rg0);
		const auto   scheme = alp::Scheme(r.u8());
		if (scheme == alp::Scheme::ALP_RD) {
			alp::state<double> stt;
			stt.scheme          = scheme;
			stt.right_bit_width = r.u8();
			stt.left_bit_width  = r.u8();
			const unsigned d    = r.u8();
			if (d > alp::config::MAX_RD_DICTIONARY_SIZE || stt.right_bit_width > 64 || stt.left_bit_width > 16) {
				throw Error("ALP_RD row group header out of range");
			}
			stt.actual_dictionary_size = uint8_t(d);
			for (unsigned i = 0; i < d; i++) stt.left_parts_dict[i] = r.u16();
			for (size_t off = 0; off < rg_n; off += VEC) {
				const size_t m = std::min(VEC, rg_n - off);
				uint16_t     c = r.u16();
				if (c > m) throw Error("more exceptions than values");
				unpack_bits(r, stt.left_bit_width, m, b.left_packed);
				unpack_bits(r, stt.right_bit_width, m, b.right_packed);
				for (size_t i = 0; i < c; i++) b.rd_exc[i] = r.u16();
				for (size_t i = 0; i < c; i++) {
					b.pos[i] = r.u16();
					if (b.pos[i] >= m) throw Error("exception position out of range");
				}
				const uint64_t zero64 = 0;
				const uint16_t zero16 = 0;
				unffor::unffor(b.right_packed, b.right, stt.right_bit_width, &zero64);
				unffor::unffor(b.left_packed, b.left, stt.left_bit_width, &zero16);
				alp::rd_encoder<double>::decode(b.out, b.right, b.left, b.rd_exc, b.pos, &c, stt);
				std::memcpy(out.data() + rg0 + off, b.out, m * sizeof(double));
			}
		} else if (scheme == alp::Scheme::ALP) {
			for (size_t off = 0; off < rg_n; off += VEC) {
				const size_t   m    = std::min(VEC, rg_n - off);
				const uint8_t  exp  = r.u8();
				const uint8_t  fac  = r.u8();
				uint16_t       c    = r.u16();
				int64_t        base = int64_t(r.u64());
				const uint8_t  bw   = r.u8();
				if (exp > alp::Constants<double>::MAX_EXPONENT || fac > exp || bw > 64 || c > m) {
					throw Error("ALP vector header out of range");
				}
				unpack_bits(r, bw, m, reinterpret_cast<uint64_t*>(b.packed));
				for (size_t i = 0; i < c; i++) {
					const uint64_t bits = r.u64();
					std::memcpy(&b.exc[i], &bits, 8);
				}
				for (size_t i = 0; i < c; i++) {
					b.pos[i] = r.u16();
					if (b.pos[i] >= m) throw Error("exception position out of range");
				}
				unffor::unffor(b.packed, b.enc, bw, &base);
				alp::decoder<double>::decode(b.enc, fac, exp, b.out);
				alp::decoder<double>::patch_exceptions(b.out, b.exc, b.pos, &c);

				generated::falp::fallback::scalar::falp(b.packed, b.out_falp, bw, &base, fac, exp);
				alp::decoder<double>::patch_exceptions(b.out_falp, b.exc, b.pos, &c);
				if (std::memcmp(b.out, b.out_falp, m * sizeof(double)) != 0) fused_agrees = false;

				std::memcpy(out.data() + rg0 + off, b.out, m * sizeof(double));
			}
		} else {
			throw Error("unknown ALP scheme " + std::to_string(int(scheme)));
		}
	}
	if (r.at != len) throw Error("bytes left over after the last vector");
	return fused_agrees;
}

// ------------------------------------------------------------------ zstd

void zstd_encode_block(const double* v, size_t n, int level, std::vector<uint8_t>& body) {
	const size_t cap = ZSTD_compressBound(n * sizeof(double));
	body.resize(cap);
	const size_t z = ZSTD_compress(body.data(), cap, v, n * sizeof(double), level);
	if (ZSTD_isError(z)) throw Error(std::string("ZSTD_compress: ") + ZSTD_getErrorName(z));
	body.resize(z);
}

void zstd_decode_block(const uint8_t* p, size_t len, std::vector<double>& out) {
	// The value count comes from the frame header's content size.
	const unsigned long long size = ZSTD_getFrameContentSize(p, len);
	if (size == ZSTD_CONTENTSIZE_ERROR || size == ZSTD_CONTENTSIZE_UNKNOWN || size % sizeof(double) != 0) {
		throw Error("zstd frame has no usable content size");
	}
	out.assign(size / sizeof(double), 0.0);
	const size_t z = ZSTD_decompress(out.data(), size, p, len);
	if (ZSTD_isError(z)) throw Error(std::string("ZSTD_decompress: ") + ZSTD_getErrorName(z));
	if (z != size) throw Error("zstd decoded a different size than its header says");
}

// ------------------------------------------------------------------ files

struct SeriesHead {
	std::string name;
	uint64_t    n;
};

[[noreturn]] void die(const std::string& msg) {
	std::fprintf(stderr, "codec-pages-alp: %s\n", msg.c_str());
	std::exit(2);
}

void read_exact(FILE* f, void* dst, size_t k, const char* what) {
	if (k && std::fread(dst, 1, k, f) != k) die(std::string("series file ends early in ") + what);
}

uint32_t read_u32(FILE* f) {
	uint8_t b[4];
	read_exact(f, b, 4, "a header");
	return uint32_t(b[0]) | uint32_t(b[1]) << 8 | uint32_t(b[2]) << 16 | uint32_t(b[3]) << 24;
}

uint64_t read_u64(FILE* f) {
	uint8_t b[8];
	read_exact(f, b, 8, "a header");
	uint64_t v = 0;
	for (int i = 7; i >= 0; i--) v = (v << 8) | b[i];
	return v;
}

// Opens a series file and reads its magic and series count.
FILE* open_series(const std::string& path, uint32_t& count) {
	FILE* f = std::fopen(path.c_str(), "rb");
	if (!f) die(path + ": " + std::strerror(errno));
	char magic[4];
	read_exact(f, magic, 4, "the magic");
	if (std::memcmp(magic, "CPS1", 4) != 0) die(path + ": not a series file");
	count = read_u32(f);
	return f;
}

SeriesHead read_head(FILE* f) {
	SeriesHead h;
	const uint32_t nl = read_u32(f);
	h.name.resize(nl);
	read_exact(f, h.name.data(), nl, "a series name");
	h.n = read_u64(f);
	return h;
}

std::vector<SeriesHead> scan(const std::string& path) {
	uint32_t                count;
	FILE*                   f = open_series(path, count);
	std::vector<SeriesHead> heads;
	for (uint32_t i = 0; i < count; i++) {
		heads.push_back(read_head(f));
		if (fseeko(f, off_t(heads.back().n * 8), SEEK_CUR) != 0) die("cannot skip a series' values");
	}
	std::fclose(f);
	return heads;
}

void write_all(FILE* f, const void* p, size_t k) {
	if (k && std::fwrite(p, 1, k, f) != k) die(std::string("write failed: ") + std::strerror(errno));
}

void write_u32(FILE* f, uint32_t v) {
	uint8_t b[4] = {uint8_t(v), uint8_t(v >> 8), uint8_t(v >> 16), uint8_t(v >> 24)};
	write_all(f, b, 4);
}

// ------------------------------------------------------------------- main

enum class Codec { ALP, ZSTD3, ZSTD19 };

void run(Codec codec, const std::string& name, const std::string& series_path, const std::string& out_dir,
         uint64_t L, const std::vector<SeriesHead>& heads) {
	uint64_t block_count = 0;
	for (const auto& h : heads) block_count += h.n / L + (h.n % L != 0);
	if (block_count > UINT32_MAX) die("too many blocks for a bodies file");

	const std::string out_path = out_dir + "/" + name + ".L" + std::to_string(L) + ".bodies";
	FILE*             out      = std::fopen(out_path.c_str(), "wb");
	if (!out) die(out_path + ": " + std::strerror(errno));
	write_all(out, "CPB1", 4);
	write_u32(out, uint32_t(block_count));

	uint32_t count;
	FILE*    in = open_series(series_path, count);

	std::vector<double>  values;
	std::vector<double>  decoded;
	std::vector<uint8_t> body;
	AlpTotals            tot;
	uint64_t             blocks = 0, total_values = 0, payload = 0, failed = 0;

	for (uint32_t s = 0; s < count; s++) {
		const SeriesHead h = read_head(in);
		for (uint64_t done = 0, bi = 0; done < h.n; bi++) {
			const size_t n = size_t(std::min<uint64_t>(L, h.n - done));
			values.resize(n);
			read_exact(in, values.data(), n * sizeof(double), "values");
			done += n;

			body.clear();
			switch (codec) {
			case Codec::ALP:
				alp_encode_block(values.data(), n, body, tot, s, bi);
				break;
			case Codec::ZSTD3:
				zstd_encode_block(values.data(), n, 3, body);
				break;
			case Codec::ZSTD19:
				zstd_encode_block(values.data(), n, 19, body);
				break;
			}
			if (body.size() > UINT32_MAX) die("a body is longer than a bodies file can say");

			uint8_t status = 1;
			try {
				bool fused_agrees = true;
				if (codec == Codec::ALP) {
					fused_agrees = alp_decode_block(body.data(), body.size(), decoded);
				} else {
					zstd_decode_block(body.data(), body.size(), decoded);
				}
				if (!fused_agrees) tot.falp_mismatch++;
				if (decoded.size() == n && std::memcmp(decoded.data(), values.data(), n * sizeof(double)) == 0) {
					status = 0;
				}
			} catch (const std::exception& e) {
				std::fprintf(stderr, "codec-pages-alp: %s series %u block %llu: %s\n", name.c_str(), s,
				             (unsigned long long)bi, e.what());
				status = 1;
			}

			write_u32(out, s);
			write_u32(out, uint32_t(n));
			write_u32(out, uint32_t(body.size()));
			write_all(out, &status, 1);
			write_all(out, body.data(), body.size());

			blocks++;
			total_values += n;
			payload += body.size();
			failed += status;
		}
	}
	std::fclose(in);
	if (std::fclose(out) != 0) die(out_path + ": " + std::strerror(errno));
	if (blocks != block_count) die("block count does not match the series file");

	std::printf("{\"codec\":\"%s\",\"block\":%llu,\"blocks\":%llu,\"values\":%llu,\"payload_bytes\":%llu,"
	            "\"failed_blocks\":%llu",
	            name.c_str(), (unsigned long long)L, (unsigned long long)blocks, (unsigned long long)total_values,
	            (unsigned long long)payload, (unsigned long long)failed);
	if (codec == Codec::ALP) {
		std::printf(",\"alp_estimate_bits\":%.3f,\"full_vector_payload_bytes\":%llu,\"alp_rowgroups\":%llu,"
		            "\"alp_rd_rowgroups\":%llu,\"exceptions\":%llu,\"falp_mismatch_blocks\":%llu,"
		            "\"library\":\"cwida/ALP %s\"",
		            tot.estimate_bits, (unsigned long long)(payload + tot.full_vector_extra),
		            (unsigned long long)tot.alp_rowgroups, (unsigned long long)tot.alp_rd_rowgroups,
		            (unsigned long long)tot.exceptions, (unsigned long long)tot.falp_mismatch, ALP_COMMIT);
	} else {
		std::printf(",\"library\":\"zstd %s\"", ZSTD_versionString());
	}
	std::printf("}\n");
	std::fflush(stdout);
}

// --verify: decodes a bodies file written earlier, in a process that never
// encoded, and checks every block against the series file again.
int verify(Codec codec, const std::string& name, const std::string& series_path, const std::string& bodies_path) {
	FILE* bf = std::fopen(bodies_path.c_str(), "rb");
	if (!bf) die(bodies_path + ": " + std::strerror(errno));
	char magic[4];
	if (std::fread(magic, 1, 4, bf) != 4 || std::memcmp(magic, "CPB1", 4) != 0) die(bodies_path + ": not a bodies file");
	const uint32_t records = read_u32(bf);

	uint32_t count;
	FILE*    in = open_series(series_path, count);
	uint32_t s  = 0;
	uint64_t left_in_series = 0;
	bool     have_series    = false;

	std::vector<double>  values, decoded;
	std::vector<uint8_t> body;
	uint64_t             failed = 0, disagree = 0;
	for (uint32_t i = 0; i < records; i++) {
		const uint32_t si  = read_u32(bf);
		const uint32_t nv  = read_u32(bf);
		const uint32_t len = read_u32(bf);
		uint8_t        stored;
		read_exact(bf, &stored, 1, "a bodies record");
		body.resize(len);
		read_exact(bf, body.data(), len, "a body");
		while (!have_series || left_in_series == 0) {
			if (have_series) s++;
			if (s >= count) die("bodies file has more blocks than the series file");
			left_in_series = read_head(in).n;
			have_series    = true;
		}
		if (si != s || nv == 0 || nv > left_in_series) die("bodies record " + std::to_string(i) + " does not follow the series");
		values.resize(nv);
		read_exact(in, values.data(), size_t(nv) * sizeof(double), "values");
		left_in_series -= nv;

		uint8_t status = 1;
		try {
			if (codec == Codec::ALP) {
				alp_decode_block(body.data(), len, decoded);
			} else {
				zstd_decode_block(body.data(), len, decoded);
			}
			if (decoded.size() == nv && std::memcmp(decoded.data(), values.data(), size_t(nv) * sizeof(double)) == 0) {
				status = 0;
			}
		} catch (const std::exception&) {
			status = 1;
		}
		failed += status;
		disagree += status != stored;
	}
	std::fclose(in);
	std::fclose(bf);
	std::printf("{\"verify\":\"%s\",\"bodies\":\"%s\",\"blocks\":%u,\"failed_blocks\":%llu,\"status_disagreements\":%llu}\n",
	            name.c_str(), bodies_path.c_str(), records, (unsigned long long)failed, (unsigned long long)disagree);
	return disagree ? 1 : 0;
}

Codec parse_codec(const std::string& name) {
	if (name == "alp") return Codec::ALP;
	if (name == "zstd3") return Codec::ZSTD3;
	if (name == "zstd19") return Codec::ZSTD19;
	die("unknown codec " + name + " (alp, zstd3, zstd19)");
}

} // namespace

int main(int argc, char** argv) {
	if (argc == 5 && std::strcmp(argv[1], "--verify") == 0) return verify(parse_codec(argv[2]), argv[2], argv[3], argv[4]);
	if (argc != 5) {
		std::fprintf(stderr,
		             "usage: %s <alp|zstd3|zstd19> <series file> <out dir> <L1,L2,...>\n"
		             "       %s --verify <alp|zstd3|zstd19> <series file> <bodies file>\n",
		             argv[0], argv[0]);
		return 2;
	}
	const std::string name  = argv[1];
	const Codec       codec = parse_codec(name);

	std::vector<uint64_t> Ls;
	for (const char* p = argv[4]; *p;) {
		char*                    end;
		const unsigned long long L = std::strtoull(p, &end, 10);
		if (end == p || L == 0 || L > UINT32_MAX) die(std::string("bad block length list: ") + argv[4]);
		Ls.push_back(L);
		p = *end == ',' ? end + 1 : end;
		if (*end && *end != ',') die(std::string("bad block length list: ") + argv[4]);
	}

	if (codec == Codec::ALP) {
		try {
			self_test_pack_bits<uint64_t>({0, 1, 2, 3, 7, 8, 13, 20, 31, 32, 33, 47, 52, 63, 64});
			self_test_pack_bits<uint16_t>({0, 1, 2, 3, 8, 15, 16});
		} catch (const std::exception& e) {
			die(e.what());
		}
	}

	const auto heads = scan(argv[2]);
	for (uint64_t L : Ls) run(codec, name, argv[2], argv[3], L, heads);
	return 0;
}
