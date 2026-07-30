#!/usr/bin/env python3
"""Locate the game's item-build route function and emit `hook-target.json`.

`src/hook.rs` detours
`<LogisticSGDAgent as AbstractItemNetwork>::get_item_builds_list`. Up to game
0.5.1 the address came from a 55-byte signature lifted out of the SDK's
`libgame_view` rlib. That is impossible from SDK 0.5.3 on: the engine rlibs ship
LLVM bitcode instead of object code, so there is no machine code to copy a
prologue from.

This script narrows the executable down instead, using the PE exception table
(`.pdata`) for exact function boundaries plus the structural fingerprint of the
target recorded below.

    # list candidates (the ABI filter alone identified the target uniquely)
    python tools/find_item_build_hook.py --exe "…/TeamfightManager2.exe"

    # once a candidate is confirmed, write the config the mod reads
    python tools/find_item_build_hook.py --exe "…" --emit 0x2155a90

`hook.rs` re-verifies whatever this writes (function start per `.pdata`, exact
prologue, unique signature) and refuses to patch if anything disagrees, so a
wrong value here fails closed rather than corrupting a match.

THE DISCRIMINATOR THAT WORKS: argument shape
--------------------------------------------
Every fingerprint taken from *emitted code* has broken on a game update. The one
taken from the *type signature* has not, because the signature is what the mod
depends on anyway:

    fn(&LogisticSGDAgent, &Vec<Box<dyn ItemInfo>>, &ChampionInfoSheet,
       &Vec<String>, &Vec<(Position, String)>, &Vec<(Position, String)>, bool)
       -> Vec<Vec<usize>>

`Vec<Vec<usize>>` is 24 bytes, so it returns via `sret` in rcx; rdx/r8/r9 take
the next three references; the remaining four arguments go on the stack at
rsp+0x28/0x30/0x38/0x40 on entry. Three are `&Vec`, which are *thin* pointers
read 8 bytes at a time, and the last is a `bool`, touched a byte at a time. So
the target reads exactly those four slots, nothing above 0x40, three as qwords
and one as a byte.

In game 0.5.3 that is true of **exactly one** of the 132,960 functions in
`.pdata`: 0x2155a90. Prologue shape alone leaves ~8000, shape + size ~2400.

CONFIRMED 0.5.3 TARGET: 0x2155a90, size 2179. Its prologue is the 0.5.2 one with
a bigger frame and an added xmm6 save, instruction for instruction otherwise:

  0.5.2  8 pushes │ sub rsp,0x158 │ lea rbp,[rsp+0x80] │ mov [rbp+0xD0],-2  │ …
  0.5.3  8 pushes │ sub rsp,0x208 │ lea rbp,[rsp+0x80] │ mov [rbp+0x168],-2 │ …

both then spilling r9/r8/rdx and keeping rcx (the sret pointer) in a callee-saved
register. It is called from four sites, one of them a 68,531-byte function — the
shape of the match simulation loop.

GROUND TRUTH, read out of SDK 0.5.2's `libgame_view` (the last SDK with real
object code), symbol
`_RNvYNtNtNtNtCs1NnncsTkPjW_9game_core…AbstractItemNetwork20get_item_builds_list…`:

  size      1869 bytes
  prologue  5541574156415541545657 53  48 81 EC 58 01 00 00  48 8D AC 24 80 …
            push rbp,r15,r14,r13,r12,rsi,rdi,rbx; sub rsp,0x158; lea rbp,[rsp+0x80]
  calls     - the SAME `spec_from_iter::<Vec<(Position, usize)>, …>` twice, both
              inside the first 0xCA bytes (team1 then team2), before any alloc
            - `<LogisticSGDAgent as AbstractItemNetwork>::calculate_item_builds`
              (600 bytes in 0.5.2, its own sibling in item_network/sgd.rs)
            - `__rust_dealloc` 7x, `raw_vec::handle_error` 2x, memcpy 2x, memmove 1x
            - `__rust_alloc` 2x, `RawVec::<Vec<usize>>::grow_one` 1x

Anchors that were tried and do NOT work, so nobody repeats them:

  * Panic `Location` structs for `game-core\\src\\simulation\\item_network\\sgd.rs`
    exist in the exe (16 of them, lines 56…643), but neither
    `get_item_builds_list` nor `calculate_item_builds` references any — they are
    panic-free. The 8 functions that do reference them are unrelated helpers.
  * There is no vtable to read: dispatch is static, and no `.rdata` slot in the
    exe points at any sgd.rs function.
  * The paired-`spec_from_iter` call fingerprint (`candidates()` below) did not
    survive 0.5.3: that callee is now inlined, and unique-callee counts dropped
    across the board. No function with the right argument shape has the pair, so
    the two filters are mutually exclusive — trust the argument shape.
  * Call-counting at runtime (`src/probe.rs`) narrows but does not decide: three
    candidates all increment once per simulated match. Two were then ruled out on
    argument shape — one takes five arguments, the other takes three `(ptr, len)`
    slice pairs by value rather than `&Vec` references.
"""

from __future__ import annotations

import argparse
import bisect
import json
import struct
import sys
from collections import Counter
from pathlib import Path

PUSHES = bytes.fromhex("554157415641554154565753")  # rbp,r15,r14,r13,r12,rsi,rdi,rbx
SIZE_RANGE = (1200, 2800)
EARLY_CALL_WINDOW = 0x120  # the paired spec_from_iter calls sit inside this
SIBLING_SIZE_RANGE = (350, 1000)  # calculate_item_builds was 600 bytes

# Signature length is grown until the bytes are unique in .text rather than
# fixed: the prologue idiom below is shared, and 40 bytes from the entry of the
# real target matches five different functions in game 0.5.3.
SIGNATURE_MIN = 40
SIGNATURE_MAX = 160

# Incoming stack-argument slots, relative to rsp on entry. With `sret` in rcx and
# the first three references in rdx/r8/r9, the remaining four arguments land
# here: &Vec<String>, &Vec<(Position, String)> x2, then the trailing bool.
ARG_SLOTS = (0x28, 0x30, 0x38, 0x40)
BOOL_SLOT = 0x40


class Image:
    def __init__(self, path: Path):
        self.data = path.read_bytes()
        d = self.data
        pe = struct.unpack_from("<I", d, 0x3C)[0]
        if d[pe : pe + 4] != b"PE\0\0":
            raise SystemExit(f"{path}: not a PE image")
        nsec = struct.unpack_from("<H", d, pe + 6)[0]
        optsz = struct.unpack_from("<H", d, pe + 20)[0]
        self.imgbase = struct.unpack_from("<Q", d, pe + 24 + 24)[0]
        self.sections = {}
        for i in range(nsec):
            o = pe + 24 + optsz + i * 40
            name = d[o : o + 8].rstrip(b"\0").decode("ascii", "replace")
            vsize, vaddr, rawsize, rawptr = struct.unpack_from("<IIII", d, o + 8)
            self.sections[name] = (vaddr, vsize, rawptr, rawsize)
        self.text = self.sections[".text"]
        self.functions = self._pdata()
        self._starts = [f[0] for f in self.functions]
        self.size_of = {b: e - b for b, e in self.functions}

    def off(self, rva: int) -> int | None:
        for vaddr, vsize, rawptr, rawsize in self.sections.values():
            if vaddr <= rva < vaddr + max(vsize, rawsize):
                return rawptr + (rva - vaddr)
        return None

    def _pdata(self):
        vaddr, vsize, rawptr, rawsize = self.sections[".pdata"]
        out = []
        for i in range(min(vsize, rawsize) // 12):
            begin, end, _unwind = struct.unpack_from("<III", self.data, rawptr + i * 12)
            if begin and end > begin:
                out.append((begin, end))
        out.sort()
        return out

    def enclosing(self, rva: int):
        i = bisect.bisect_right(self._starts, rva) - 1
        if i >= 0:
            begin, end = self.functions[i]
            if begin <= rva < end:
                return begin, end
        return None

    def in_text(self, rva: int) -> bool:
        vaddr, vsize, _rawptr, rawsize = self.text
        return vaddr <= rva < vaddr + min(vsize, rawsize)

    def code(self, rva: int, length: int) -> bytes:
        start = self.off(rva)
        return self.data[start : start + length] if start is not None else b""

    def calls(self, begin: int, end: int):
        """(offset_from_start, target_rva) for each direct E8 call."""
        base = self.off(begin)
        span = self.data[base : base + (end - begin)]
        out = []
        for i in range(len(span) - 4):
            if span[i] == 0xE8:
                target = begin + i + 5 + struct.unpack_from("<i", span, i + 1)[0]
                if self.in_text(target):
                    out.append((i, target))
        return out

    def occurrences(self, needle: bytes) -> int:
        vaddr, vsize, rawptr, rawsize = self.text
        blob = self.data[rawptr : rawptr + min(vsize, rawsize)]
        count = start = 0
        while (found := blob.find(needle, start)) != -1:
            count += 1
            start = found + 1
        return count


def stack_arg_widths(img: Image, begin: int, end: int) -> dict[int, set[int]]:
    """Incoming stack-arg slot -> set of access widths, for one function.

    Walks the function linearly tracking `rsp` through push/pop/sub/add and
    `rbp` once it is set from `rsp`, so every `[rsp+d]` / `[rbp+d]` operand can
    be expressed relative to `rsp` on entry. Offsets at or above 0x28 are the
    caller's argument area (0x00 is the return address and 0x08..0x20 the shadow
    space), so they identify the callee's arity, and the access width separates
    a pointer argument from a `bool`.

    Only the linear walk is tracked, which is sound here because the target
    establishes a frame pointer in its prologue and addresses everything through
    `rbp` from then on.
    """
    import capstone

    md = capstone.Cs(capstone.CS_ARCH_X86, capstone.CS_MODE_64)
    md.detail = True
    X = capstone.x86

    rsp, rbp, have_rbp = 0, None, False
    widths: dict[int, set[int]] = {}
    for ins in md.disasm(img.code(begin, end - begin), begin):
        mn, ops = ins.mnemonic, ins.operands
        if mn == "push":
            rsp -= 8
        elif mn == "pop":
            rsp += 8
        elif (
            mn in ("sub", "add")
            and ops
            and ops[0].type == X.X86_OP_REG
            and ins.reg_name(ops[0].reg) == "rsp"
            and ops[1].type == X.X86_OP_IMM
        ):
            rsp += -ops[1].imm if mn == "sub" else ops[1].imm
        elif (
            mn == "lea"
            and ops
            and ins.reg_name(ops[0].reg) == "rbp"
            and ops[1].type == X.X86_OP_MEM
            and ins.reg_name(ops[1].mem.base) == "rsp"
        ):
            rbp, have_rbp = rsp + ops[1].mem.disp, True
        for o in ops:
            if o.type != X.X86_OP_MEM or o.mem.index != 0:
                continue
            base = ins.reg_name(o.mem.base) if o.mem.base else None
            if base == "rsp":
                off = rsp + o.mem.disp
            elif base == "rbp" and have_rbp:
                off = rbp + o.mem.disp
            else:
                continue
            if off >= ARG_SLOTS[0] and off % 8 == 0:
                widths.setdefault(off, set()).add(o.size)
    return widths


def matches_abi(widths: dict[int, set[int]]) -> bool:
    """Whether a function's stack-argument use matches `get_item_builds_list`.

    Four stack arguments and no more (so `max` is the bool slot), the first
    three read as 8-byte pointers, the last touched a byte at a time because it
    is a `bool`. This is the one fingerprint taken from the *type signature*
    rather than from emitted code, so it is the only one that has survived a
    game update unchanged — see the module docstring.
    """
    if not set(ARG_SLOTS) <= set(widths) or max(widths) > ARG_SLOTS[-1]:
        return False
    if not all(8 in widths[slot] for slot in ARG_SLOTS if slot != BOOL_SLOT):
        return False
    return 1 in widths[BOOL_SLOT]


def abi_candidates(img: Image):
    """Functions whose prologue, size and argument use all fit the target."""
    out = []
    for begin, end in img.functions:
        size = end - begin
        if not (SIZE_RANGE[0] <= size <= SIZE_RANGE[1]):
            continue
        if not (img.in_text(begin) and img.in_text(end - 1)):
            continue
        if img.code(begin, len(PUSHES)) != PUSHES:
            continue
        if not matches_abi(stack_arg_widths(img, begin, end)):
            continue
        calls = img.calls(begin, end)
        out.append(
            {
                "rva": begin,
                "size": size,
                "calls": len(calls),
                "unique_callees": len(Counter(t for _off, t in calls)),
            }
        )
    out.sort(key=lambda c: abs(c["size"] - 1869))
    return out


def candidates(img: Image):
    out = []
    for begin, end in img.functions:
        size = end - begin
        if not (SIZE_RANGE[0] <= size <= SIZE_RANGE[1]):
            continue
        if not (img.in_text(begin) and img.in_text(end - 1)):
            continue
        if img.code(begin, len(PUSHES)) != PUSHES:
            continue

        calls = img.calls(begin, end)
        counts = Counter(t for _off, t in calls)
        paired_early = [
            t
            for t, n in counts.items()
            if n == 2 and all(off < EARLY_CALL_WINDOW for off, tt in calls if tt == t)
        ]
        repeated = [t for t, n in counts.items() if n >= 6]  # __rust_dealloc
        sibling = [
            t
            for t in counts
            if (f := img.enclosing(t))
            and SIBLING_SIZE_RANGE[0] <= f[1] - f[0] <= SIBLING_SIZE_RANGE[1]
        ]
        if not (paired_early and repeated and sibling):
            continue

        # Closeness to the 0.5.2 shape, purely to order the list.
        score = -abs(size - 1869) - 50 * abs(len(counts) - 9)
        out.append(
            {
                "rva": begin,
                "size": size,
                "calls": len(calls),
                "unique_callees": len(counts),
                "paired_early": paired_early,
                "sibling": sibling[:3],
                "score": score,
            }
        )
    out.sort(key=lambda c: -c["score"])
    return out


def emit(img: Image, rva: int, out_path: Path) -> int:
    func = img.enclosing(rva)
    if not func or func[0] != rva:
        print(f"error: {rva:#x} is not a function start per .pdata", file=sys.stderr)
        return 1
    prologue = img.code(rva, len(PUSHES))
    if prologue != PUSHES:
        print(
            f"error: prologue at {rva:#x} is {prologue.hex()}, expected {PUSHES.hex()}",
            file=sys.stderr,
        )
        return 1
    # Grow the signature until it is unique. The shared prologue idiom means a
    # fixed 40 bytes is not enough: in game 0.5.3 that prefix matches five
    # functions, all of which push the same registers and set up the same frame.
    signature = b""
    for length in range(SIGNATURE_MIN, SIGNATURE_MAX + 1, 8):
        signature = img.code(rva, length)
        if img.occurrences(signature) == 1:
            break
    else:
        print(
            f"error: no signature up to {SIGNATURE_MAX} bytes is unique in .text",
            file=sys.stderr,
        )
        return 1
    payload = {
        "_comment": (
            "Generated by tools/find_item_build_hook.py. hook.rs prefers 'rva'; "
            "'signature' survives a rebase of the image and is re-checked for "
            "uniqueness at load. Both are verified against .pdata before patching."
        ),
        "rva": hex(rva),
        "signature": signature.hex(),
        "size": func[1] - func[0],
    }
    out_path.write_text(json.dumps(payload, indent=2) + "\n", encoding="utf-8")
    print(f"wrote {out_path}")
    print(f"  rva       {rva:#x}  (size {func[1] - func[0]})")
    print(f"  signature {signature.hex()}")
    print(f"            ({len(signature)} bytes, unique in .text)")
    return 0


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--exe", required=True, type=Path, help="TeamfightManager2.exe")
    ap.add_argument(
        "--emit",
        metavar="RVA",
        help="confirmed function start (e.g. 0x10591f0); writes hook-target.json",
    )
    ap.add_argument(
        "--out",
        type=Path,
        default=Path("hook-target.json"),
        help="output path (default: ./hook-target.json)",
    )
    ap.add_argument(
        "--legacy",
        action="store_true",
        help="use the pre-0.5.3 call-pattern filter instead of argument shape",
    )
    args = ap.parse_args()

    img = Image(args.exe)
    print(
        f"{args.exe.name}: {len(img.functions)} functions in .pdata, "
        f"image base {img.imgbase:#x}"
    )

    if args.emit:
        return emit(img, int(args.emit, 0), args.out)

    if not args.legacy:
        try:
            found = abi_candidates(img)
        except ImportError:
            print(
                "\ncapstone is not installed, falling back to the legacy "
                "call-pattern filter (which does not decide — see --help).\n"
                "  pip install capstone\n",
                file=sys.stderr,
            )
            args.legacy = True
        else:
            print(f"\n{len(found)} candidates by argument shape "
                  f"(prologue + size + args 0x28/0x30/0x38 = &Vec, 0x40 = bool):\n")
            for c in found:
                print(f"  rva={c['rva']:#09x} size={c['size']:<5} "
                      f"calls={c['calls']:<3} uniq={c['unique_callees']}")
            if len(found) == 1:
                print(
                    f"\nUnique match. Confirm it still behaves (the mod logs "
                    f"hook_installed, and builds should follow item-builds.json), "
                    f"then:\n  --emit {found[0]['rva']:#x}"
                )
            else:
                print(
                    "\nNot unique — the argument shape has stopped discriminating. "
                    "Re-read the module docstring before trusting anything here."
                )
            return 0

    found = candidates(img)
    print(f"\n{len(found)} candidates (prologue + size + call-pattern match):\n")
    for c in found:
        print(f"  rva={c['rva']:#09x} size={c['size']:<5} calls={c['calls']:<3} "
              f"uniq={c['unique_callees']:<3} "
              f"paired_early={[hex(t) for t in c['paired_early']]} "
              f"sibling={[hex(t) for t in c['sibling']]}")
    print(
        "\nThis list is NOT conclusive — see the module docstring. This filter "
        "found ~54 candidates in 0.5.3 and none of them was the target; prefer "
        "the argument-shape filter."
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
