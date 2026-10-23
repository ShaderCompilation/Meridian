#!/usr/bin/env python3
"""Add a large amount of demo data to a Meridian database (for performance testing).
Usage: tools/gen_demo.py <db> [tasks_per_section]"""
import random, sqlite3, sys, time
db = sqlite3.connect(sys.argv[1]); n = int(sys.argv[2]) if len(sys.argv) > 2 else 40
random.seed(7)
c = db.cursor()
nid = (c.execute("select max(m) from (select max(id) m from boards union all select max(id) from sections union all select max(id) from tasks union all select max(id) from subtasks union all select max(id) from labels)").fetchone()[0] or 0) + 1
def new():
    global nid; nid += 1; return nid
words = "fix add refactor review write ship design test deploy update migrate document plan research polish improve investigate".split()
nouns = "login flow, settings page, sync engine, onboarding, release notes, API client, dark mode, search index, billing, dashboard, database schema, CI pipeline, error handling, landing page".split(", ")
for b in range(3):
    bid = new(); c.execute("insert into boards values (?,?,?)", (bid, f"Project {chr(65+b)}", 10000 + b * 1024))
    labels = []
    for i, name in enumerate(["frontend", "backend", "bug", "design", "infra"]):
        lid = new(); labels.append(lid); c.execute("insert into labels values (?,?,?,?)", (lid, bid, name, i))
    for s, sname in enumerate(["Backlog", "Todo", "Planned", "In Progress", "Review", "Done"]):
        sid = new(); c.execute("insert into sections values (?,?,?,?,0)", (sid, bid, sname, (s + 1) * 1024))
        for t in range(n):
            tid = new()
            title = f"{random.choice(words).capitalize()} {random.choice(nouns)}"
            due = None if random.random() < 0.5 else time.strftime("%Y-%m-%d", time.localtime(time.time() + random.randint(-5, 30) * 86400))
            c.execute("insert into tasks values (?,?,?,?,?,?,?,?,?)", (tid, sid, title, "Some details " * random.randint(0, 6), random.randint(0, 4), due, (t + 1) * 1024, int(time.time()), int(time.time()) if sname == "Done" else None))
            for k in range(random.choice([0, 0, 2, 3, 5])):
                c.execute("insert into subtasks values (?,?,?,?,?)", (new(), tid, f"Step {k+1}", random.random() < 0.5, k))
            for i, l in enumerate(random.sample(labels, random.choice([0, 1, 1, 2]))):
                c.execute("insert into task_labels values (?,?,?)", (tid, l, i))
db.commit()
print("tasks:", c.execute("select count(*) from tasks").fetchone()[0])
