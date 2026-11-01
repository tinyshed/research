// One pass of PISA over a query file, as the round's other engines run one:
// each query once, timed from its term numbers to its ten, after a warm pass
// of every query when asked. PISA's own `queries` tool warms every posting
// list first and times each query as the mean of two runs after an untimed
// one, which no other engine here does; this driver uses PISA's index,
// cursors, scorer and query algorithms exactly as that tool builds them.
//
// Usage: pass <index> <wand> <queries.pisa> <results.jsonl> <algorithm> <answer|warm|cold>
//
// The index is block_simdbp, the WAND data raw with fixed blocks, BM25 with
// k1 1.2 and b 0.75; a query is unweighted, each distinct term once. An
// algorithm is block_max_maxscore, block_max_wand or ranked_or, the last
// scoring every posting.

#include <chrono>
#include <cstdio>
#include <fstream>
#include <iostream>
#include <sstream>
#include <string>
#include <vector>

#include <spdlog/spdlog.h>

#include "cursor/block_max_scored_cursor.hpp"
#include "cursor/scored_cursor.hpp"
#include "index_types.hpp"
#include "memory_source.hpp"
#include "query.hpp"
#include "query/algorithm/block_max_maxscore_query.hpp"
#include "query/algorithm/block_max_wand_query.hpp"
#include "query/algorithm/ranked_or_query.hpp"
#include "scorer/scorer.hpp"
#include "topk_queue.hpp"
#include "util/do_not_optimize_away.hpp"
#include "wand_data.hpp"
#include "wand_data_raw.hpp"

using namespace pisa;

int main(int argc, char** argv) {
    if (argc != 7) {
        std::cerr << "pass <index> <wand> <queries.pisa> <results.jsonl> <algorithm> <answer|warm|cold>\n";
        return 2;
    }
    // PISA's library logs to standard output, which carries this driver's line.
    spdlog::set_level(spdlog::level::err);
    std::string const index_path = argv[1];
    std::string const wand_path = argv[2];
    std::string const algorithm = argv[5];
    std::string const mode = argv[6];
    std::vector<Query> queries;
    {
        std::ifstream input(argv[3]);
        std::string line;
        while (std::getline(input, line)) {
            auto colon = line.find(':');
            std::vector<std::uint32_t> ids;
            std::istringstream terms(line.substr(colon + 1));
            std::uint32_t id;
            while (terms >> id) {
                ids.push_back(id);
            }
            queries.emplace_back(line.substr(0, colon), ids);
        }
    }
    auto opening = std::chrono::steady_clock::now();
    using Wand = wand_data<wand_data_raw>;
    Wand wdata(MemorySource::mapped_file(wand_path));
    ScorerParams params("bm25");
    params.bm25_k1 = 1.2F;
    params.bm25_b = 0.75F;
    auto scorer = scorer::from_params(params, wdata);
    run_for_index("block_simdbp", MemorySource::mapped_file(index_path), [&](auto index) {
        auto run = [&](Query const& query) {
            topk_queue topk(10);
            if (algorithm == "block_max_maxscore") {
                block_max_maxscore_query q(topk);
                q(make_block_max_scored_cursors(index, wdata, *scorer, query, false), index.num_docs());
            } else if (algorithm == "block_max_wand") {
                block_max_wand_query q(topk);
                q(make_block_max_scored_cursors(index, wdata, *scorer, query, false), index.num_docs());
            } else if (algorithm == "ranked_or") {
                ranked_or_query q(topk);
                q(make_scored_cursors(index, *scorer, query, false), index.num_docs());
            } else {
                throw std::invalid_argument("no algorithm " + algorithm);
            }
            topk.finalize();
            return topk.topk();
        };
        double open_ms =
            std::chrono::duration<double, std::milli>(std::chrono::steady_clock::now() - opening).count();
        if (mode == "warm") {
            for (auto const& query: queries) {
                do_not_optimize_away(run(query).size());
            }
        } else if (mode != "cold" && mode != "answer") {
            throw std::invalid_argument("a pass is answer, warm or cold");
        }
        std::FILE* out = std::fopen(argv[4], "w");
        for (auto const& query: queries) {
            auto started = std::chrono::steady_clock::now();
            auto top = run(query);
            auto ns = std::chrono::duration_cast<std::chrono::nanoseconds>(
                          std::chrono::steady_clock::now() - started
            )
                          .count();
            std::string id(query.id().value_or(""));
            if (mode == "answer") {
                std::fprintf(out, "{\"qid\":\"%s\",\"ten\":[", id.c_str());
                for (std::size_t at = 0; at < top.size(); ++at) {
                    std::fprintf(
                        out, "%s[%u,%.9g]", at == 0 ? "" : ",", unsigned(top[at].second), double(top[at].first)
                    );
                }
                std::fprintf(out, "],\"us\":%.3f}\n", ns / 1e3);
            } else {
                std::fprintf(out, "{\"qid\":\"%s\",\"ns\":%lld,\"ids\":[", id.c_str(), (long long)ns);
                for (std::size_t at = 0; at < top.size(); ++at) {
                    std::fprintf(out, "%s%u", at == 0 ? "" : ",", unsigned(top[at].second));
                }
                std::fprintf(out, "]}\n");
            }
        }
        std::fclose(out);
        std::printf(
            "{\"step\":\"%s\",\"engine\":\"pisa-%s\",\"mode\":\"%s\",\"queries\":%zu,\"open_ms\":%.3f}\n",
            mode == "answer" ? "answer" : "time",
            algorithm.c_str(),
            mode.c_str(),
            queries.size(),
            open_ms
        );
    });
}
