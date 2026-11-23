# pack-probe

What the tables of a pack cost a hub: the domain lists and address ranges people would put in one, compiled
into a text of ours, loaded, held in memory and looked up; and what three sets of icons weigh. The standard
library alone, so that a number is the data's and no crate's.

| Table     | From                                                 | Licence   | Found by               |
|-----------|------------------------------------------------------|-----------|------------------------|
| `domains` | `v2fly/domain-list-community`, its `data/` as text   | MIT       | a suffix at a label    |
| `country` | DB-IP's IP to Country Lite, CSV                      | CC BY 4.0 | the range an address is in |
| `asn`     | DB-IP's IP to ASN Lite, CSV                          | CC BY 4.0 | the range an address is in |

## How to run it

```sh
mkdir -p data && cd data
git clone --depth 1 https://github.com/v2fly/domain-list-community geosite
git clone --depth 1 https://github.com/simple-icons/simple-icons
git clone --depth 1 https://github.com/gilbarbara/logos
git clone --depth 1 https://github.com/lipis/flag-icons flags
for f in dbip-country-lite dbip-asn-lite; do
  curl -fsS "https://download.db-ip.com/free/$f-$(date +%Y-%m).csv.gz" | gunzip > "$f.csv"
done
cd .. && cargo build --release && ./target/release/pack-probe data
cargo build --release --features store && ./target/release/stored data   # the same kept in TinyStore
```

`data/` is not committed. The probe writes `data/out/*.tsv`, the text a pack would carry, prints a row a
table, and says on stderr what each source held.

## What it does not measure

A regular expression: the lists' 163 `regexp:` rules are counted and left out. The static binary's weight, which a table's reader of forty lines does not move.

## What a pack's files cost the binary

`weigh/` is a crate of its own: `rows` keeps rows as a hub did, `rows_and_files` keeps a pack's files beside
them with the calls Dashbin makes. `weigh.sh` builds both static for musl, in the image Dashbin's `just size`
builds in, and prints their bytes.
