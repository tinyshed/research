#!/bin/sh
set -eu
cp /target/release/search-lab /target/release/search-lab-full
for profile in baseline icu auto detect jieba tantivy;do
 case "$profile" in
  baseline) features='' ;;
  icu) features='icu' ;;
  auto) features='stems' ;;
  detect) features='detect' ;;
  jieba) features='jieba' ;;
  tantivy) features='stems,text' ;;
 esac
 if [ -z "$features" ];then cargo build --release --locked --no-default-features
 else cargo build --release --locked --no-default-features --features "$features";fi
 cp /target/release/search-lab "/target/release/search-lab-$profile"
done
cp /target/release/search-lab-full /target/release/search-lab
python3 - <<'PY'
import datetime,hashlib,json,pathlib,subprocess
profiles=['baseline','icu','auto','detect','jieba','tantivy','full']
r={'revision':subprocess.check_output(['git','rev-parse','HEAD'],text=True).strip(),'utc':datetime.datetime.now(datetime.timezone.utc).isoformat(),'command':'sh sizes.sh','settings':'release opt-level=3, thin LTO, codegen-units=1, strip=symbols; same reachable research CLI','artifacts':{p:{'bytes':(f:=pathlib.Path('/target/release/search-lab-'+p)).stat().st_size,'sha256':hashlib.sha256(f.read_bytes()).hexdigest()}for p in profiles},'boundary':'Whole Linux ELF executables, not projected TinyStore binary deltas. ICU compiled segmenter dictionaries/models and Jieba compressed dictionary included; no runtime download.'}
pathlib.Path('../reports/data/search-2026-10-16/binary-sizes.json').write_text(json.dumps(r,indent=2)+'\n')
PY
