set -e
cd <repo>/lemmesee
step() { echo "=== $1 $(date -u +%FT%TZ)"; }
step "round-1k product_look"; python round-1k/product_look.py corpus/round-1k/product
step "round-1k product_teachers"; python round-1k/product_teachers.py corpus/round-1k/product
step "round-1l build"; python round-1l/ranker.py build corpus/round-1c/teacher corpus/round-1l/teacher corpus/round-1l/learn
step "round-1l fit"; python round-1l/ranker.py fit corpus/round-1l/learn
step "round-1l cv"; python round-1l/ranker.py cv corpus/round-1l/learn
step "round-1l judged"; python round-1l/ranker.py judged corpus/round-1l/learn corpus/see corpus/round-1j | grep -v ' questions$'
step "round-1m roles"; python round-1m/roles.py count corpus/round-1m/roles
step "round-1m bench count"; python round-1m/bench.py count corpus/round-1m/bench corpus/round-1l/learn
step "round-1m bench reach"; python round-1m/bench.py reach corpus/round-1m/bench
step "done"
python -c "import sys, numpy, lightgbm, sklearn, scipy; print(sys.version.split()[0], numpy.__version__, lightgbm.__version__, sklearn.__version__, scipy.__version__)"
