"""Round 1t: schemas written by language models in many house styles.

    python -I teach.py assign <out>
    python -I teach.py collect <out>

`assign` draws, with a fixed seed, each teacher's (domain, style)
assignments from DOMAINS and teacher.md's styles and writes the filled
prompts, one prompt of eight assignments an agent, into
<out>/prompts/teach-<teacher>/ for Claude Haiku 5.5 and Claude Sonnet 5.5 at
medium effort, both asked as agents of Claude Code, and their SHA-256 into
<out>/prompts/teach-hashes.txt. `collect` reads every agent's file in
<out>/teacher/<teacher>/, keeps each schema whose keys all resolve, and writes <out>/teacher/
generated.jsonl in round 1b's form, split by domain, with what was refused
counted in <out>/teacher/collect.json.
"""

import hashlib
import json
import random
import re
import sys
from collections import Counter
from pathlib import Path

HERE = Path(__file__).resolve().parent
SEED = 20261108

STYLES = {}
for line in (HERE / "teacher.md").read_text(encoding="utf-8").splitlines():
    m = re.match(r"^\| ([a-z ]+) \| (.+) \|$", line)
    if m and m.group(1) != "Name" and not m.group(1).startswith("-"):
        STYLES[m.group(1)] = m.group(2)
PROMPT = re.search(r"## The prompt\n\n```\n(.*?)\n```", (HERE / "teacher.md").read_text(encoding="utf-8"), re.S).group(1)

# Domains of every kind, none of LiveSQLBench-Base-Full-v1's 22
DOMAINS = """hospital admissions; dental clinic; veterinary practice; pharmacy inventory; clinical trials;
blood bank; ambulance dispatch; fitness club memberships; yoga studio bookings; university enrollment;
school timetabling; online course platform; public library circulation; journal peer review; academic conference
management; research grant office; patent office; law firm case management; court scheduling; prison management;
police incident reports; fire department operations; municipal building permits; property tax assessment;
voter registration; parliament votes; embassy visa processing; customs declarations; airline reservations;
airport ground handling; railway timetables; bus fleet maintenance; ride hailing; car rental; car dealership;
auto repair shop; parking garages; electric vehicle charging network; container port terminal; freight forwarding;
warehouse management; last-mile delivery; food delivery app; restaurant kitchen orders; industrial bakery;
craft brewery; winery; coffee roastery; grocery chain; farmers cooperative; dairy farm; fishing quotas; forestry
harvest planning; open-pit mining; oil well drilling; power grid outages; water utility billing; telecom billing;
internet service provider; mobile network towers; data center assets; software bug tracker; continuous
integration builds; feature flag service; SaaS subscription billing; CRM sales pipeline; marketing campaigns;
affiliate network; online ad auctions; e-commerce marketplace; auction house; pawn shop; jewelry manufacturing;
textile factory; furniture workshop; semiconductor fab; aircraft maintenance; shipyard; construction projects;
real estate listings; property management; hotel reservations; hostel chain; cruise line; theme park; cinema
chain; theatre productions; concert ticketing; music streaming; podcast network; video game studio; esports
league; chess federation; golf club; ski resort; marathon timing; zoo; botanical garden; public aquarium;
wildlife tracking; weather station network; seismology network; astronomical observatory; satellite operations;
drone fleet; payroll; recruiting and applicant tracking; expense reports; general ledger accounting; bank loans;
credit card disputes; car insurance policies; pension fund; stock brokerage accounts; microfinance; charity
donations; parish registry; wedding planning; funeral home; genealogy; dating app; social network; recipe
sharing site; translation agency; print shop; newspaper newsroom; television broadcast schedule; film
production; art gallery sales; recycling plant; waste collection routes; elevator maintenance; smart home
devices; retail point of sale; loyalty program; call center tickets; IT asset inventory; laboratory information
system; genome sequencing lab; hospital pharmacy dispensing; nursing home care; home care visits; physiotherapy
clinic; optician; hearing aid fitting; medical device recalls; food safety inspections; restaurant reviews;
tourism tour operator; travel expense booking; car sharing; bicycle sharing; scooter rental; public transit
fares; toll roads; road maintenance; bridge inspections; railway freight; pipeline monitoring; nuclear plant
maintenance; wind farm operations; hydro dam operations; district heating; gas distribution; propane delivery;
pest control services; cleaning company; security guard scheduling; alarm monitoring; locksmith jobs; plumbing
service calls; HVAC servicing; solar water heaters; garden centre; florist; pet grooming; animal shelter
adoptions; horse racing; stud farm breeding; beekeeping; vineyard harvest; greenhouse climate control;
hydroponic farm; seed bank; flour mill; sugar refinery; meat processing plant; seafood exporter; spice trading;
tea plantation; cocoa cooperative; cotton gin; rubber plantation; timber yard; paper mill; cement plant; steel
mill; foundry; glass works; chemical plant batches; paint manufacturer; cosmetics brand; perfume house;
toy manufacturer; sporting goods retailer; bicycle manufacturer; motorcycle dealer; boat marina; yacht
charter; flight school; driving school; language school; music school; dance studio; martial arts dojo;
scouting association; trade union membership; professional certification body; standards organisation; trade
fair exhibitors; coworking spaces; event venue rentals; catering company; vending machines; laundromat chain;
dry cleaning; tailor shop; shoe repair; watch repair; antiques dealer; rare book seller; comic book store;
board game cafe; escape rooms; bowling alley; karaoke bars; nightclub guest lists; casino floor; lottery
operator; sports betting; fantasy sports league; amateur football league; swimming club; rowing club; sailing
regatta; mountain rescue; coast guard; military logistics; veterans benefits; social housing allocation;
unemployment benefits; child care centres; foster care placements; adoption agency; immigration case files;
refugee resettlement; food bank; homeless shelter; public health vaccinations; disease surveillance; poison
control centre; dental insurance claims; health insurance claims; workers compensation; occupational safety
incidents; environmental permits; air quality monitoring; noise complaints; urban tree inventory; cemetery
plots; land registry; cadastral survey; mining claims; water rights; fishing licences; hunting permits; national
park visitors; campground reservations; mountain huts; tourist information centre; city bike lanes; snow
removal; street lighting; traffic signals; parking enforcement; vehicle registration; driver licensing;
taxi licensing; court fines; debt collection; credit bureau; payday lending; currency exchange bureau; money
transfer service; crowdfunding platform; venture capital fund; private equity deals; mergers and acquisitions;
corporate board meetings; shareholder registry; company formation agent; tax return preparation; audit firm
engagements; management consulting projects; architecture firm; interior design studio; engineering
consultancy; surveying company; geotechnical lab; materials testing; quality control lab; calibration
services; metrology institute; patent licensing; trademark registry; domain name registrar; web hosting;
email newsletter service; customer feedback surveys; market research panels; polling company; census
operations; statistical office; open data portal; museum-free heritage register; archive records management;
document scanning bureau; translation memory; subtitles studio; dubbing studio; voice actor agency; talent
agency; modelling agency; photography studio; stock photo marketplace; printing on demand; merchandise store;
fan club; ticket resale; stadium operations; arena concessions; sports medicine clinic; athlete doping
controls; referee assignments; tournament brackets; league standings; youth academy scouting""".replace("\n", " ")
DOMAINS = [d.strip() for d in DOMAINS.split(";") if d.strip()]

TEACHERS = {"haiku": 24, "sonnet": 18}  # agents of eight assignments each
EACH = 8


def prompt(assignments) -> str:
    lines = "\n".join(f"- domain: {d}; house style: {STYLES[s]}" for d, s in assignments)
    return PROMPT.replace("{count}", str(len(assignments))).replace("{assignments}", lines)


def assign(out: Path) -> None:
    rng = random.Random(SEED)
    styles = sorted(STYLES)
    domains = DOMAINS[:]
    rng.shuffle(domains)
    # Every domain once, every style used about as often by each teacher
    hashes, start = [], 0
    for teacher, agents in TEACHERS.items():
        n = agents * EACH
        rows = [(domains[start + i], styles[(i + start) % len(styles)]) for i in range(n)]
        start += n
        name, each = f"teach-{teacher}", EACH
        d = out / "prompts" / name
        d.mkdir(parents=True, exist_ok=True)
        for k in range(0, len(rows), each):
            text = prompt(rows[k:k + each])
            f = d / f"{name}-{k // each:03d}.txt"
            f.write_text(text, encoding="utf-8")
            hashes.append(f"{hashlib.sha256(text.encode()).hexdigest()}  {name}/{f.name}")
        (d / "assignments.json").write_text(json.dumps(rows, indent=0) + "\n", encoding="utf-8")
    (out / "prompts" / "teach-hashes.txt").write_text("\n".join(hashes) + "\n", encoding="utf-8")
    print(f"{len(STYLES)} styles, {len(DOMAINS)} domains, {start} assigned")


FAMILY = [(r"^(small|big)?serial|^(tiny|small|medium|big)?int|^integer", "int"), (r"^uuid", "uuid"),
          (r"^(var)?char|^character|^text|^citext|^string|^nvarchar|^nchar", "text")]


def family(t: str) -> str:
    t = t.strip().lower()
    for pattern, f in FAMILY:
        if re.match(pattern, t):
            return f
    return "other"


def last_json(text: str):
    """The last whole JSON object of a reply: a model may write two, or fence one."""
    text = re.sub(r"```(?:json)?", "", text)
    starts = [m.start() for m in re.finditer(r"\{\s*\"schemas\"", text)]
    for st in reversed(starts):
        depth, inside, esc = 0, False, False
        for i in range(st, len(text)):
            c = text[i]
            if inside:
                esc = (c == "\\") and not esc
                if c == '"' and not esc:
                    inside = False
                continue
            if c == '"':
                inside = True
            elif c == "{":
                depth += 1
            elif c == "}":
                depth -= 1
                if depth == 0:
                    try:
                        return json.loads(text[st:i + 1])
                    except json.JSONDecodeError:
                        break
    return None


def convert(raw, teacher: str, sid: str, refused: Counter):
    """One teacher schema in round 1b's form, or None with the reason counted."""
    tables, names = [], {}
    for t in raw.get("tables", []):
        if not isinstance(t, dict) or not t.get("name") or not t.get("columns"):
            refused["a table without a name or columns"] += 1
            return None
        if t["name"].lower() in names:
            refused["two tables of one name"] += 1
            return None
        cols, seen = [], set()
        pk = [p for p in (t.get("primary_key") or []) if isinstance(p, str)]
        for i, c in enumerate(t["columns"]):
            if not isinstance(c, dict) or not c.get("name") or c["name"].lower() in seen:
                continue
            seen.add(c["name"].lower())
            typ = str(c.get("type") or "text")
            cols.append({"name": c["name"], "family": family(typ), "type": typ.lower(),
                         "nullable": bool(c.get("nullable", True)) and c["name"] not in pk,
                         "unique": bool(c.get("unique", False)), "primary": c["name"] in pk,
                         "index": False, "position": i})
        if not pk or any(p.lower() not in seen for p in pk):
            refused["a primary key not among the columns"] += 1
            return None
        names[t["name"].lower()] = {"name": t["name"], "pk": pk, "columns": cols}
        tables.append(names[t["name"].lower()])
    if len(tables) < 3:
        refused["fewer than three tables"] += 1
        return None
    fk = []
    for k in raw.get("foreign_keys", []):
        try:
            a, c, b, rc = names[k["table"].lower()], k["column"], names[k["ref_table"].lower()], k["ref_column"]
        except (KeyError, AttributeError, TypeError):
            refused["a key naming a table not there (key dropped)"] += 1
            continue
        col = next((x for x in a["columns"] if x["name"].lower() == str(c).lower()), None)
        ref = next((x for x in b["columns"] if x["name"].lower() == str(rc).lower()), None)
        if col is None or ref is None:
            refused["a key naming a column not there (key dropped)"] += 1
            continue
        fk.append([a["name"], col["name"], b["name"]])
    if not fk:
        refused["no key that resolves"] += 1
        return None
    domain = str(raw.get("domain", ""))
    return {"id": sid, "repo": f"{teacher}/{domain}", "source": f"teacher-{teacher}", "licence": "",
            "domain": domain, "style": str(raw.get("style", "")), "tables": tables,
            "fk": sorted({tuple(x) for x in fk}), "closed": [], "novel": False}


def split_of(domain: str) -> str:
    """A domain's split, so that no domain sits on two sides."""
    h = int(hashlib.sha256(domain.encode()).hexdigest(), 16) % 10
    return "train" if h < 7 else ("validation" if h < 8 else "test")


def collect(out: Path) -> None:
    refused = Counter()
    rows = []
    replies = [(t, f) for t in TEACHERS for f in sorted((out / "teacher" / t).glob("*.json"))]
    assigned = {}
    for name in (f"teach-{t}" for t in TEACHERS):
        p = out / "prompts" / name / "assignments.json"
        if p.exists():
            for d, s in json.load(open(p, encoding="utf-8")):
                assigned[d.lower()] = s
    for teacher, f in replies:
        text = f.read_text(encoding="utf-8")
        obj = last_json(text)
        if obj is None:
            refused["a reply without a whole JSON object"] += 1
            continue
        for k, raw in enumerate(obj.get("schemas", [])):
            s = convert(raw, teacher, f"{teacher}/{f.stem}/{k}", refused)
            if s is None:
                continue
            s["style"] = assigned.get(s["domain"].lower(), s["style"])
            s["split"] = split_of(s["domain"].lower())
            rows.append(s)
    with open(out / "teacher" / "generated.jsonl", "w", encoding="utf-8") as w:
        for s in rows:
            w.write(json.dumps(s, ensure_ascii=False) + "\n")
    summary = {"replies": len(replies), "schemas": len(rows), "keys": sum(len(s["fk"]) for s in rows),
               "by teacher": Counter(s["source"] for s in rows), "by split": Counter(s["split"] for s in rows),
               "by style": Counter(s["style"] for s in rows), "refused": refused}
    (out / "teacher" / "collect.json").write_text(json.dumps(summary, indent=1) + "\n", encoding="utf-8")
    print(json.dumps(summary, indent=1))


if __name__ == "__main__":
    sys.stdout.reconfigure(encoding="utf-8")
    {"assign": assign, "collect": collect}[sys.argv[1]](Path(sys.argv[2]))
