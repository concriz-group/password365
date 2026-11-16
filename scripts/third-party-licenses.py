"""Lists the third-party packages an app ships (Rust crates and, for the
desktop, its JavaScript packages) with each one's licence text, for the
in-app "Third-party licenses" screen.

    python3 scripts/third-party-licenses.py <Cargo.toml> [<package.json dir>] <out.json>

Identical licence texts are stored once. Workspace crates (ours) are left out.
"""
import hashlib, json, os, re, subprocess, sys

FILES = re.compile(r"^(licen[cs]e|copying|notice|copyright)(-|_|\.|$)", re.I)

def texts_in(folder):
    if not folder or not os.path.isdir(folder):
        return []
    out = []
    for name in sorted(os.listdir(folder)):
        path = os.path.join(folder, name)
        if FILES.match(name) and os.path.isfile(path):
            try:
                out.append(open(path, encoding="utf-8", errors="replace").read().strip())
            except OSError:
                pass
    return out

def rust(manifest):
    meta = json.loads(subprocess.check_output(
        ["cargo", "metadata", "--format-version", "1", "--manifest-path", manifest], text=True))
    local = set(meta["workspace_members"])
    # Only what the app links: normal dependencies reachable from the workspace, any platform.
    nodes = {n["id"]: n for n in meta["resolve"]["nodes"]}
    shipped, todo = set(), list(local)
    while todo:
        node = nodes[todo.pop()]
        for dep in node["deps"]:
            if any(k["kind"] in (None, "normal") for k in dep["dep_kinds"]) and dep["pkg"] not in shipped:
                shipped.add(dep["pkg"]); todo.append(dep["pkg"])
    packages = []
    for p in meta["packages"]:
        if p["id"] in shipped and p["id"] not in local and p.get("source"):
            folder = os.path.dirname(p["manifest_path"])
            packages.append(("rust", p["name"], p["version"], p.get("license") or "see licence text",
                             texts_in(folder) or ([open(os.path.join(folder, p["license_file"])).read()] if p.get("license_file") else [])))
    return packages

def javascript(folder):
    tree = json.loads(subprocess.run(["npm", "ls", "--omit=dev", "--all", "--json"], cwd=folder,
                                     capture_output=True, text=True).stdout or "{}")
    seen, out = set(), []
    def walk(deps):
        for name, info in (deps or {}).items():
            key = (name, info.get("version"))
            if key in seen or not info.get("version"):
                continue
            seen.add(key)
            pkg_dir = os.path.join(folder, "node_modules", name)
            try:
                pkg = json.load(open(os.path.join(pkg_dir, "package.json")))
            except OSError:
                pkg = {}
            licence = pkg.get("license") if isinstance(pkg.get("license"), str) else "see licence text"
            out.append(("javascript", name, info["version"], licence, texts_in(pkg_dir)))
            walk(info.get("dependencies"))
    walk(tree.get("dependencies"))
    return out

args = sys.argv[1:]
manifest, out_path = args[0], args[-1]
packages = rust(manifest) + (javascript(args[1]) if len(args) == 3 else [])
texts, rows = {}, []
for ecosystem, name, version, licence, found in sorted(packages, key=lambda p: (p[1].lower(), p[2])):
    ids = []
    for text in found:
        key = hashlib.sha256(text.encode()).hexdigest()[:12]
        texts.setdefault(key, text)
        ids.append(key)
    rows.append({"name": name, "version": version, "license": licence, "ecosystem": ecosystem, "texts": ids})
json.dump({"packages": rows, "texts": texts}, open(out_path, "w"), ensure_ascii=False, separators=(",", ":"))
missing = [r["name"] for r in rows if not r["texts"]]
print(f"{len(rows)} packages, {len(texts)} distinct licence texts, {os.path.getsize(out_path) // 1024} KB; without a licence file: {len(missing)}")
