#!/usr/bin/env python3
"""Reference implementation of `slice` using Python's own slicing.

Reads stdin line by line, splits each line on whitespace, and evaluates
`cols[<expr>]` using Python's eval (with `cols` as the only binding).

Index out of range is treated like Rust's lenient version: the line yields
an empty result instead of raising. Bad expressions exit 2 with a message.
"""

import sys


def main() -> int:
    if len(sys.argv) != 2:
        sys.stderr.write("usage: reference.py <expr>\n")
        return 2
    expr = sys.argv[1]

    # Validate by compiling once; report a parse error similarly to the Rust CLI.
    try:
        code = compile(f"cols[{expr}]", "<expr>", "eval")
    except SyntaxError as e:
        sys.stderr.write(f"reference.py: parse error: {e}\n")
        return 2

    out = sys.stdout
    for raw in sys.stdin:
        # Strip the trailing newline only; .split() with no args already
        # handles all internal whitespace.
        cols = raw.split()
        try:
            result = eval(code, {"__builtins__": {}}, {"cols": cols})
        except IndexError:
            out.write("\n")
            continue
        except TypeError as e:
            # e.g. cols[0, 1] -> tuple subscript on a list
            sys.stderr.write(f"reference.py: {e}\n")
            return 2

        if isinstance(result, list):
            out.write(" ".join(result))
        else:
            out.write(str(result))
        out.write("\n")
    return 0


if __name__ == "__main__":
    sys.exit(main())
