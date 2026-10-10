# The teeth of rule 7 (caps.sh): fed one crate's sources and its tests,
# prints each WGSL shader no test can be shown to reach. Plain POSIX awk
# (mawk, gawk, busybox).
#
# A string holding an entry point (@vertex, @fragment, @compute) must sit
# in an item that carries a shader: a const or static of &str, or a fn
# giving a String or a &str. A test reaches an item when its code says
# the item's name; whatever a reached item's code says is reached in turn
# (a shader put together from parts). Only code counts, never comments or
# strings, and only those items: a test's `Validator::new` reaches no
# `pub fn new` that happens to sit beside a shader. Tests are the files
# under tests/ and, in a source, all after its first #[cfg(test)].

BEGIN {
  n = split("new default from into clone fmt to_string as_str", w, " ")
  for (k = 1; k <= n; k++) stop[w[k]] = 1
}

FNR == 1 && NR > 1 { file(prev, text) }
FNR == 1 { prev = FILENAME; text = "" }
{ text = text $0 "\n" }
END {
  if (NR > 0) file(prev, text)
  judge()
}

function ident(c) { return c != "" && c ~ /[A-Za-z0-9_]/ }

# One file: a test whole, or a source up to its tests and those after.
function file(f, s,    at, head, t) {
  if (f ~ /(^|\/)tests\//) {
    tests++
    scan(f, s, 1, 1)
    return
  }
  at = index(s, "#[cfg(test)]")
  if (at == 0) {
    scan(f, s, 0, 1)
    return
  }
  head = substr(s, 1, at - 1)
  scan(f, head, 0, 1)
  tests++
  t = head
  scan(f, substr(s, at), 1, gsub(/\n/, "", t) + 1)
}

# Walk `s` (from line `line` of `f`) as Rust: code, comments, strings
# (plain and raw), char literals. Outside a test it finds the items above,
# where each ends, the names its code says and the strings in it that
# hold an entry point; in a test, the names its code says.
function scan(f, s, test, line,    n, i, c, p, mode, depth, hashes, shut, from, sline, wfrom, word, item, kind, idepth, rest, name, sig, j, k) {
  n = length(s)
  mode = "code"
  depth = 0
  item = ""
  for (i = 1; i <= n; i++) {
    c = substr(s, i, 1)
    if (c == "\n") line++
    if (mode == "line") {
      if (c == "\n") mode = "code"
      continue
    }
    if (mode == "block") {
      if (c == "*" && substr(s, i + 1, 1) == "/") { mode = "code"; i++ }
      continue
    }
    if (mode == "str") {
      if (c == "\\") {
        i++
        if (substr(s, i, 1) == "\n") line++
      } else if (c == "\"") {
        mode = "code"
        said(f, substr(s, from, i - from), sline, item, test)
      }
      continue
    }
    if (mode == "raw") {
      if (c == "\"" && substr(s, i + 1, hashes) == shut) {
        mode = "code"
        said(f, substr(s, from, i - from), sline, item, test)
        i += hashes
      }
      continue
    }
    p = i > 1 ? substr(s, i - 1, 1) : ""
    if (c == "/" && substr(s, i + 1, 1) == "/") { mode = "line"; i++; continue }
    if (c == "/" && substr(s, i + 1, 1) == "*") { mode = "block"; i++; continue }
    if (c == "\"") { mode = "str"; from = i + 1; sline = line; continue }
    if (c == "r" && (!ident(p) || (p == "b" && !ident(substr(s, i - 2, 1)))) && match(substr(s, i + 1, 9), /^#*"/)) {
      hashes = RLENGTH - 1
      shut = substr("########", 1, hashes)
      mode = "raw"
      i += RLENGTH
      from = i + 1
      sline = line
      continue
    }
    if (c == "'") {
      # A char ('x', '\n', '\'', '\u{..}'), or else a lifetime.
      if (substr(s, i + 1, 1) == "\\") {
        j = index(substr(s, i + 3), "'")
        if (j) { i += j + 2; continue }
      } else if (substr(s, i + 2, 1) == "'") { i += 2; continue }
    }
    if (item == "" && !test && !ident(p) && c ~ /[a-z]/) {
      rest = substr(s, i, 300)
      if (match(rest, /^(pub(\([a-z]+\))? +)?(const|static) +[A-Z][A-Z0-9_]* *: *& *('static +)?str[^A-Za-z0-9_]/)) {
        name = rest
        sub(/^(pub(\([a-z]+\))? +)?(const|static) +/, "", name)
        sub(/[^A-Z0-9_].*$/, "", name)
        item = name; kind = "const"; idepth = depth
        is[name] = 1
      } else if (match(rest, /^fn +[a-z_][a-z0-9_]*/)) {
        name = substr(rest, 1, RLENGTH)
        sub(/^fn +/, "", name)
        j = index(rest, "{")
        k = index(rest, ";")
        sig = j && (!k || j < k) ? substr(rest, 1, j - 1) : ""
        if (sig ~ /->[ \n]*(String|&[ \n]*('static[ \n]+)?str)[ \n]*$/) {
          item = name; kind = "fn"; idepth = depth
          is[name] = 1
        }
      }
    }
    if (ident(c)) {
      if (!ident(p)) wfrom = i
      if (!ident(substr(s, i + 1, 1))) {
        word = substr(s, wfrom, i - wfrom + 1)
        if (test) seen[word] = 1
        else if (item != "") says[item SUBSEP word] = 1
      }
    }
    if (c == "{") depth++
    if (c == "}") depth--
    if (item != "" && depth == idepth && ((kind == "fn" && c == "}") || (kind == "const" && c == ";"))) item = ""
  }
}

# A string, as it closes: an entry point in it marks the item it is in,
# or is loose (in nothing a test could name).
function said(f, str, sline, item, test) {
  if (test || str !~ /@(vertex|fragment|compute)/) return
  if (item != "") shader[f SUBSEP item] = 1
  else loose[f ":" sline] = 1
}

function judge(    name, key, kf, changed, any) {
  changed = 1
  while (changed) {
    changed = 0
    for (name in is) {
      if ((name in done) || !(name in seen) || (name in stop)) continue
      done[name] = 1
      changed = 1
      for (key in says) {
        split(key, kf, SUBSEP)
        if (kf[1] == name) seen[kf[2]] = 1
      }
    }
  }
  for (key in shader) {
    any = 1
    split(key, kf, SUBSEP)
    if (!(kf[2] in done)) print kf[1] ": " kf[2] " holds a shader no naga test reaches"
  }
  for (key in loose) {
    any = 1
    print key ": a shader in no const or fn giving a string, so no test can name it"
  }
  if (any && !tests) print crate " has shaders and no tests to check them with naga"
}
