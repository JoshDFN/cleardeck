#!/usr/bin/env python3
"""Compare two Candid interfaces STRUCTURALLY, the way the wire compares them.

WHY THIS EXISTS
---------------
`.did` drift was guarded by `diff -u committed.did extracted.did`, and that guard
was left as `exit 0` with a TODO for a whole release cycle. The reason it could
never be turned on is that a byte diff is the WRONG COMPARISON for these files:

  * `src/table_canister/table_canister.did` carries several hundred lines of
    hand-written documentation that `candid-extractor` does not emit -- and
    those comments are not decoration, they are published on-chain as the
    canister's public `candid:service` metadata. Byte-diffing against the
    extractor output would have demanded deleting all of it.
  * The extractor numbers anonymous `Result_N` aliases in its own order, so an
    unrelated edit renumbers dozens of them. `Result_2` here and `Result_7`
    there can denote exactly the same type.
  * Field order inside a record is not part of a Candid record's identity.
  * `blob` and `vec nat8` are the same type.

So a byte diff is simultaneously too loud (it screams about all of the above)
and, being disabled because it was too loud, entirely silent about the drift
that matters. This tool compares what actually reaches a client: for every
method, the FULLY RESOLVED argument and result types, with aliases inlined,
fields sorted, and `blob` normalised. Comments and ordering cannot affect it,
and a missing field or a missing method cannot hide from it.

WHAT COUNTS AS A DIFFERENCE
---------------------------
A method present in one interface and not the other; a method whose resolved
signature differs at any depth; a difference in the service's init arguments.
Nothing else.

USAGE
    candid_compare.py A.did B.did --label-a COMMITTED --label-b WASM
    candid_compare.py A.did B.did --list      # one stable line per difference
Exit status is 1 when the interfaces differ, 0 when they agree.
"""
import argparse
import json
import re
import sys

TOKEN = re.compile(
    r'''(?P<ws>\s+|//[^\n]*)
      | (?P<arrow>->)
      | (?P<punc>[{}()<>;:,=])
      | (?P<str>"(?:[^"\\]|\\.)*")
      | (?P<id>[A-Za-z_][A-Za-z0-9_]*)
      | (?P<num>[0-9]+)''',
    re.VERBOSE,
)

PRIM = {
    'nat', 'nat8', 'nat16', 'nat32', 'nat64', 'int', 'int8', 'int16', 'int32',
    'int64', 'float32', 'float64', 'bool', 'text', 'null', 'reserved', 'empty',
    'principal',
}
MODIFIERS = ('query', 'oneway', 'composite_query')


class CandidSyntaxError(Exception):
    pass


def lex(src):
    toks, i = [], 0
    while i < len(src):
        m = TOKEN.match(src, i)
        if not m:
            raise CandidSyntaxError(f'cannot lex at: {src[i:i + 40]!r}')
        i = m.end()
        if m.lastgroup != 'ws':
            toks.append(m.group(0))
    return toks


class Parser:
    def __init__(self, toks):
        self.t, self.i = toks, 0

    def peek(self, k=0):
        j = self.i + k
        return self.t[j] if j < len(self.t) else None

    def next(self):
        v = self.peek()
        self.i += 1
        return v

    def eat(self, v):
        if self.peek() != v:
            raise CandidSyntaxError(
                f'expected {v!r}, found {self.peek()!r} near {" ".join(self.t[self.i:self.i + 8])}')
        return self.next()

    def opt(self, v):
        if self.peek() == v:
            self.next()
            return True
        return False


def _unquote(name):
    return name[1:-1] if name.startswith('"') else name


def parse_type(p):
    t = p.peek()
    if t in ('record', 'variant'):
        kind = p.next()
        p.eat('{')
        fields, pos = [], 0
        while p.peek() != '}':
            if p.peek(1) == ':':
                name = _unquote(p.next())
                p.eat(':')
                ty = parse_type(p)
            elif kind == 'variant' and p.peek(1) in (';', '}'):
                name, ty = _unquote(p.next()), ('null',)
            else:
                # A field with no name is positional: record { text; nat64 }.
                # Its Candid key is its INDEX, so the index must be preserved.
                ty, name, pos = parse_type(p), str(pos), pos + 1
            fields.append((name, ty))
            if not p.opt(';'):
                break
        p.eat('}')
        return (kind, fields)
    if t in ('opt', 'vec'):
        p.next()
        return (t, parse_type(p))
    if t == 'blob':
        p.next()
        return ('vec', ('nat8',))          # blob IS vec nat8
    if t == 'func':
        p.next()
        return ('func', parse_func(p))
    if t == 'service':
        p.next()
        if p.peek() == '{':
            depth = 0
            while True:
                x = p.next()
                if x == '{':
                    depth += 1
                elif x == '}':
                    depth -= 1
                    if depth == 0:
                        break
        return ('service',)
    if t in PRIM:
        p.next()
        return (t,)
    if t and re.fullmatch(r'[A-Za-z_][A-Za-z0-9_]*', t):
        p.next()
        return ('ref', t)
    raise CandidSyntaxError(f'unexpected token in type position: {t!r}')


def parse_arglist(p):
    p.eat('(')
    args = []
    while p.peek() != ')':
        if p.peek(1) == ':':      # named argument: (n : nat64)
            p.next()
            p.eat(':')
        args.append(parse_type(p))
        if not p.opt(','):
            break
    p.eat(')')
    return args


def parse_func(p):
    a = parse_arglist(p)
    p.eat('->')
    r = parse_arglist(p)
    mods = []
    while p.peek() in MODIFIERS:
        mods.append(p.next())
    return (a, r, sorted(mods))


def parse_file(path):
    p = Parser(lex(open(path, encoding='utf-8').read()))
    types, methods, init = {}, {}, None
    while p.peek() is not None:
        if p.peek() == 'type':
            p.next()
            name = p.next()
            p.eat('=')
            types[name] = parse_type(p)
            p.opt(';')
        elif p.peek() == 'service':
            p.next()
            if p.peek() != ':':
                p.next()               # optional service name
            p.eat(':')
            if p.peek() == '(':        # init args, e.g. service : (TableConfig) -> {..}
                init = parse_arglist(p)
                p.eat('->')
            p.eat('{')
            while p.peek() != '}':
                name = _unquote(p.next())
                p.eat(':')
                methods[name] = parse_func(p)
                p.opt(';')
            p.eat('}')
            p.opt(';')
        else:
            p.next()
    return types, methods, init


def resolve(ty, types, stack):
    """Inline every alias. A cycle becomes a positional back-reference, so two
    interfaces that use different NAMES for the same recursive type agree."""
    kind = ty[0]
    if kind == 'ref':
        name = ty[1]
        if name in stack:
            return ['@rec', len(stack) - stack.index(name)]
        if name not in types:
            return ['@undefined', name]
        return resolve(types[name], types, stack + [name])
    if kind in ('record', 'variant'):
        return [kind, sorted([[n, resolve(t, types, stack)] for n, t in ty[1]],
                             key=lambda x: x[0])]
    if kind in ('opt', 'vec'):
        return [kind, resolve(ty[1], types, stack)]
    if kind == 'func':
        a, r, m = ty[1]
        return ['func', [resolve(x, types, stack) for x in a],
                [resolve(x, types, stack) for x in r], m]
    return [kind]


def signature(fn, types):
    a, r, m = fn
    return {'args': [resolve(x, types, []) for x in a],
            'rets': [resolve(x, types, []) for x in r],
            'mods': m}


def kind_of(t):
    return t[0] if isinstance(t, list) and t else repr(t)


def path_diff(a, b, path, out):
    """Report the MINIMAL differing paths between two resolved types, so the
    message names the one missing field instead of dumping two type trees."""
    if a == b:
        return out
    ka, kb = kind_of(a), kind_of(b)
    if ka != kb:
        out.append((path, json.dumps(a)[:160], json.dumps(b)[:160]))
        return out
    if ka in ('record', 'variant'):
        fa, fb = {n: t for n, t in a[1]}, {n: t for n, t in b[1]}
        for n in sorted(set(fa) - set(fb)):
            out.append((f'{path}.{n}', 'present: ' + json.dumps(fa[n])[:110], 'ABSENT'))
        for n in sorted(set(fb) - set(fa)):
            out.append((f'{path}.{n}', 'ABSENT', 'present: ' + json.dumps(fb[n])[:110]))
        for n in sorted(set(fa) & set(fb)):
            path_diff(fa[n], fb[n], f'{path}.{n}', out)
        return out
    if ka in ('opt', 'vec'):
        return path_diff(a[1], b[1], path + ('[]' if ka == 'vec' else '?'), out)
    out.append((path, json.dumps(a)[:160], json.dumps(b)[:160]))
    return out


def method_paths(fa, fb, ta, tb):
    aa, ra, ma = fa
    ab, rb, mb = fb
    out = []
    if ma != mb:
        out.append(('<annotation>', ' '.join(ma) or '(update)', ' '.join(mb) or '(update)'))
    if len(aa) != len(ab):
        out.append(('<arity of arguments>', str(len(aa)), str(len(ab))))
    if len(ra) != len(rb):
        out.append(('<arity of results>', str(len(ra)), str(len(rb))))
    for i, (x, y) in enumerate(zip(aa, ab)):
        path_diff(resolve(x, ta, []), resolve(y, tb, []), f'arg{i}', out)
    for i, (x, y) in enumerate(zip(ra, rb)):
        path_diff(resolve(x, ta, []), resolve(y, tb, []), f'ret{i}', out)
    return out


def compare(path_a, path_b):
    ta, ma, ia = parse_file(path_a)
    tb, mb, ib = parse_file(path_b)
    findings = []
    for name in sorted(set(ma) - set(mb)):
        findings.append(('method-only-in-a', name, []))
    for name in sorted(set(mb) - set(ma)):
        findings.append(('method-missing-from-a', name, []))
    for name in sorted(set(ma) & set(mb)):
        if json.dumps(signature(ma[name], ta), sort_keys=True) != \
           json.dumps(signature(mb[name], tb), sort_keys=True):
            findings.append(('signature', name, method_paths(ma[name], mb[name], ta, tb)))
    if (ia or []) or (ib or []):
        sa = json.dumps([resolve(x, ta, []) for x in (ia or [])], sort_keys=True)
        sb = json.dumps([resolve(x, tb, []) for x in (ib or [])], sort_keys=True)
        if sa != sb:
            findings.append(('init-args', '<service>', [('<init>', sa[:160], sb[:160])]))
    return findings


def main():
    ap = argparse.ArgumentParser(description=__doc__,
                                 formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument('a')
    ap.add_argument('b')
    ap.add_argument('--label-a', default='A')
    ap.add_argument('--label-b', default='B')
    ap.add_argument('--list', action='store_true',
                    help='one stable line per difference, for baselining')
    args = ap.parse_args()

    try:
        findings = compare(args.a, args.b)
    except CandidSyntaxError as e:
        # A .did that does not parse is itself drift: `icp build` embeds this
        # file verbatim as candid:service, so a malformed one ships.
        print(f'CANDID SYNTAX ERROR: {e}', file=sys.stderr)
        sys.exit(2)

    if args.list:
        for kind, name, paths in findings:
            if not paths:
                print(f'{kind}\t{name}')
            for p, _va, _vb in paths:
                print(f'{kind}\t{name}\t{p}')
        sys.exit(1 if findings else 0)

    for kind, name, paths in findings:
        if kind == 'method-only-in-a':
            print(f'\nMETHOD DECLARED BUT NOT IMPLEMENTED: {name}')
            print(f'   present in {args.label_a}, absent from {args.label_b}')
        elif kind == 'method-missing-from-a':
            print(f'\nMETHOD IMPLEMENTED BUT NOT DECLARED: {name}')
            print(f'   present in {args.label_b}, absent from {args.label_a}')
        else:
            print(f'\nSIGNATURE DIFFERS: {name}')
        for p, va, vb in paths:
            print(f'   {p}')
            print(f'      {args.label_a}: {va}')
            print(f'      {args.label_b}: {vb}')

    print(f'\nstructural differences: {len(findings)}')
    sys.exit(1 if findings else 0)


if __name__ == '__main__':
    main()
