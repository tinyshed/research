"""The services a small application runs without TinyStore, each started as
its vendor ships it, in a directory of its own, on this machine's loopback or
a Unix socket. A setting is changed only where the round needs the same
durability from both sides, or where the service would refuse the round's
load; each such change is named here, beside the reason.
"""
import datetime
import hashlib
import hmac
import os
import shutil
import socket
import subprocess
import time
import urllib.error
import urllib.request
from pathlib import Path

MINIO_USER = "bench"
MINIO_SECRET = "bench-secret-key"
BUCKET = "bench"


class Service:
    def __init__(self, name, process, ready_seconds, address, stop):
        self.name = name
        self.process = process
        self.ready_seconds = ready_seconds
        self.address = address
        self._stop = stop

    def stop(self):
        self._stop(self)


def _terminate(service, timeout=60):
    if service.process.poll() is None:
        service.process.terminate()
        try:
            service.process.wait(timeout)
        except subprocess.TimeoutExpired:
            service.process.kill()
            service.process.wait()


def _wait(probe, process, name, limit=120.0):
    """Seconds from now until probe() is true, polled every 10 ms."""
    started = time.monotonic()
    while time.monotonic() - started < limit:
        if process.poll() is not None:
            raise RuntimeError(f"{name} exited with {process.returncode} before it was ready")
        try:
            if probe():
                return time.monotonic() - started
        except (OSError, urllib.error.URLError, ConnectionError):
            pass
        time.sleep(0.01)
    raise RuntimeError(f"{name} was not ready within {limit} s")


def _http_ok(url):
    def probe():
        with urllib.request.urlopen(url, timeout=1) as answer:
            return answer.status == 200
    return probe


def _unix_ping(path, ping=b"PING\r\n", want=b"+PONG"):
    def probe():
        with socket.socket(socket.AF_UNIX) as s:
            s.settimeout(1)
            s.connect(path)
            s.sendall(ping)
            return s.recv(64).startswith(want)
    return probe


def _log(directory, name):
    return open(directory / f"{name}.log", "wb")


def postgres(directory: Path):
    """PostgreSQL 18 as initdb makes it: its defaults, synchronous_commit on.
    A Unix socket and no TCP; max_connections raised from 100 to 200 for the
    round's 64 callers, its worker and the next run's pool."""
    data = directory / "pg"
    sockets = directory / "pg-socket"
    data.mkdir(parents=True)
    sockets.mkdir(parents=True)
    shutil.chown(data, "postgres", "postgres")
    shutil.chown(sockets, "postgres", "postgres")
    subprocess.run(["runuser", "-u", "postgres", "--", "initdb", "-D", str(data), "-U", "bench",
                    "--auth=trust", "--no-sync"], check=True, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    process = subprocess.Popen(
        ["runuser", "-u", "postgres", "--", "postgres", "-D", str(data), "-c", "listen_addresses=",
         "-c", f"unix_socket_directories={sockets}", "-c", "max_connections=200"],
        stdout=_log(directory, "postgres"), stderr=subprocess.STDOUT)
    ready = _wait(lambda: subprocess.run(["pg_isready", "-q", "-h", str(sockets), "-U", "bench"]).returncode == 0,
                  process, "postgres")
    subprocess.run(["runuser", "-u", "postgres", "--", "psql", "-q", "-h", str(sockets), "-U", "bench",
                    "-d", "postgres", "-c", "create database bench"], check=True, stdout=subprocess.DEVNULL)

    def stop(service):
        # postgres runs under runuser, which passes SIGTERM on: a smart shutdown
        subprocess.run(["runuser", "-u", "postgres", "--", "pg_ctl", "stop", "-D", str(data), "-m", "fast", "-w"],
                       stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        _terminate(service)
    return Service("postgres", process, ready, str(sockets), stop), [data]


def _redis_like(binary, name, directory: Path, durability):
    """Redis and Valkey as their binaries start without a config file, their
    snapshots included, with the append-only file the round's durability
    asks: `synced` is appendfsync always, `written` appendfsync everysec,
    `default` none, as the vendor ships it."""
    data = directory / name
    data.mkdir(parents=True)
    path = str(directory / f"{name}.sock")
    command = [binary, "--port", "0", "--unixsocket", path, "--unixsocketperm", "777", "--dir", str(data)]
    if durability == "synced":
        command += ["--appendonly", "yes", "--appendfsync", "always"]
    elif durability == "written":
        command += ["--appendonly", "yes", "--appendfsync", "everysec"]
    process = subprocess.Popen(command, stdout=_log(directory, name), stderr=subprocess.STDOUT)
    ready = _wait(_unix_ping(path), process, name)
    return Service(name, process, ready, path, _terminate), [data]


def redis(directory, durability):
    return _redis_like("redis-server", "redis", directory, durability)


def valkey(directory, durability):
    return _redis_like("valkey-server", "valkey", directory, durability)


def _sign(method, url_path, host, region="us-east-1"):
    """AWS Signature V4 for a request without a body: MinIO's bucket is made
    with one, since nothing here ships an S3 client that creates buckets."""
    now = datetime.datetime.now(datetime.UTC)
    stamp, day = now.strftime("%Y%m%dT%H%M%SZ"), now.strftime("%Y%m%d")
    payload = hashlib.sha256(b"").hexdigest()
    headers = f"host:{host}\nx-amz-content-sha256:{payload}\nx-amz-date:{stamp}\n"
    signed = "host;x-amz-content-sha256;x-amz-date"
    canonical = f"{method}\n{url_path}\n\n{headers}\n{signed}\n{payload}"
    scope = f"{day}/{region}/s3/aws4_request"
    to_sign = f"AWS4-HMAC-SHA256\n{stamp}\n{scope}\n{hashlib.sha256(canonical.encode()).hexdigest()}"
    key = f"AWS4{MINIO_SECRET}".encode()
    for part in (day, region, "s3", "aws4_request"):
        key = hmac.new(key, part.encode(), hashlib.sha256).digest()
    signature = hmac.new(key, to_sign.encode(), hashlib.sha256).hexdigest()
    return {
        "x-amz-date": stamp, "x-amz-content-sha256": payload,
        "Authorization": f"AWS4-HMAC-SHA256 Credential={MINIO_USER}/{scope}, SignedHeaders={signed}, "
                         f"Signature={signature}",
    }


def minio(directory: Path):
    """MinIO's last release on one drive, as `minio server <dir>` starts it."""
    data = directory / "minio"
    data.mkdir(parents=True)
    env = dict(os.environ, MINIO_ROOT_USER=MINIO_USER, MINIO_ROOT_PASSWORD=MINIO_SECRET,
               MINIO_BROWSER="off", MINIO_UPDATE="off")
    process = subprocess.Popen(["minio", "server", str(data), "--address", "127.0.0.1:9000", "--quiet"],
                               env=env, stdout=_log(directory, "minio"), stderr=subprocess.STDOUT)
    ready = _wait(_http_ok("http://127.0.0.1:9000/minio/health/ready"), process, "minio")
    # ready answers before the drive takes writes: a bucket made at once may get 503
    def bucket():
        request = urllib.request.Request(f"http://127.0.0.1:9000/{BUCKET}", method="PUT",
                                         headers=_sign("PUT", f"/{BUCKET}", "127.0.0.1:9000"))
        try:
            urllib.request.urlopen(request, timeout=10).close()
            return True
        except urllib.error.HTTPError as error:
            if error.code == 503:
                return False
            raise
    ready += _wait(bucket, process, "minio's bucket")
    return Service("minio", process, ready, "http://127.0.0.1:9000", _terminate), [data]


def victoria(directory: Path, scrape=None):
    """VictoriaMetrics single-node as its binary starts, scraping the
    application's /metrics when the round gives a target."""
    data = directory / "victoria"
    data.mkdir(parents=True)
    command = ["victoria-metrics", "-storageDataPath", str(data), "-httpListenAddr", "127.0.0.1:8428",
               "-loggerLevel", "ERROR"]
    if scrape:
        config = directory / "victoria-scrape.yml"
        config.write_text(f"scrape_configs:\n  - job_name: app\n    scrape_interval: 10s\n"
                          f"    static_configs:\n      - targets: ['{scrape}']\n")
        command += ["-promscrape.config", str(config)]
    process = subprocess.Popen(command, stdout=_log(directory, "victoria"), stderr=subprocess.STDOUT)
    ready = _wait(_http_ok("http://127.0.0.1:8428/health"), process, "victoria-metrics")
    return Service("victoria-metrics", process, ready, "http://127.0.0.1:8428", _terminate), [data]


def prometheus(directory: Path):
    """Prometheus 3 taking remote writes, which it refuses unless asked, and
    scraping nothing: the round writes its samples as an agent would."""
    data = directory / "prometheus"
    data.mkdir(parents=True)
    config = directory / "prometheus.yml"
    config.write_text("global:\n  scrape_interval: 15s\n")
    process = subprocess.Popen(
        ["prometheus", f"--config.file={config}", f"--storage.tsdb.path={data}",
         "--web.listen-address=127.0.0.1:9090", "--web.enable-remote-write-receiver", "--log.level=error"],
        stdout=_log(directory, "prometheus"), stderr=subprocess.STDOUT)
    ready = _wait(_http_ok("http://127.0.0.1:9090/-/ready"), process, "prometheus")
    return Service("prometheus", process, ready, "http://127.0.0.1:9090", _terminate), [data]


LOKI = """auth_enabled: false
server:
  http_listen_address: 127.0.0.1
  http_listen_port: 3100
  grpc_listen_address: 127.0.0.1
  grpc_listen_port: 9096
  log_level: warn
common:
  instance_addr: 127.0.0.1
  path_prefix: {data}
  storage:
    filesystem:
      chunks_directory: {data}/chunks
      rules_directory: {data}/rules
  replication_factor: 1
  ring:
    kvstore:
      store: inmemory
schema_config:
  configs:
    - from: 2020-10-24
      store: tsdb
      object_store: filesystem
      schema: v13
      index:
        prefix: index_
        period: 24h
# Loki's defaults refuse a tenant past 4 MiB a second: raised so that the
# round's ingest is measured rather than refused.
limits_config:
  ingestion_rate_mb: 4096
  ingestion_burst_size_mb: 4096
  per_stream_rate_limit: 4096MB
  per_stream_rate_limit_burst: 4096MB
  max_entries_limit_per_query: 100000
analytics:
  reporting_enabled: false
"""


def loki(directory: Path):
    """Loki 3 as one binary on the filesystem, as its getting-started config
    has it, with the ingestion limits raised (see LOKI)."""
    data = directory / "loki"
    data.mkdir(parents=True)
    config = directory / "loki.yml"
    config.write_text(LOKI.format(data=data))
    process = subprocess.Popen(["loki", f"-config.file={config}"], stdout=_log(directory, "loki"),
                               stderr=subprocess.STDOUT)
    ready = _wait(_http_ok("http://127.0.0.1:3100/ready"), process, "loki")
    return Service("loki", process, ready, "http://127.0.0.1:3100", _terminate), [data]


TINYSTORE = Path(__file__).resolve().parent / "node_modules/@tinyshed/tinystore-linux-x64/bin/tinystore"


def tinystore(directory: Path):
    """TinyStore's server as a person starts it, `tinystore serve <dir>`: the
    directory's sidecar, which `open(dir)` finds through its SERVE file."""
    data = directory / "store"
    data.mkdir(parents=True)
    process = subprocess.Popen([str(TINYSTORE), "serve", str(data)], stdout=_log(directory, "tinystore"),
                               stderr=subprocess.STDOUT)
    path = str(data / "server" / "tinystore.sock")

    def probe():
        with socket.socket(socket.AF_UNIX) as s:
            s.settimeout(1)
            s.connect(path)
            return True
    ready = _wait(probe, process, "tinystore")
    return Service("tinystore", process, ready, str(data), _terminate), [data]
