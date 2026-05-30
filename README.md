# slice

Writing awk / sed / cut commands to slice columnar text is quite esoteric for me and I tend to have to keep looking up the syntax.

On the other hand, I've been personally fine-tuned on Python slicing grammar. This project is a CLI that lets me use that.

## Example

Given `file.txt` looks like this:

```text
name age city status
alice 32 paris active
bob 28 tokyo inactive
```

Desired output: every column from the third onward:

```text
city status
paris active
tokyo inactive
```

With standard shell tools:

```sh
# cut is 1-indexed and needs an explicit delimiter
cat file.txt | cut -d ' ' -f 3-

# If there are exactly 4 columns, awk can name both output fields
cat file.txt | awk '{ print $3, $4 }'

# Otherwise a for loop is needed for open-ended ranges.
cat file.txt | awk '{ for (i = 3; i <= NF; i++) printf "%s%s", $i, (i == NF ? ORS : OFS) }'
```

With `slice`, use the same Python-style slice you would write for a list:

```sh
cat file.txt | slice '2:'
```

The mental model:

```python
# file.txt
inp = [
    ["name", "age", "city", "status"],
    ["alice", "32", "paris", "active"],
    ["bob", "28", "tokyo", "inactive"],
]

for row in inp:
    # The `2:` slice (zero-indexed) goes here to return
    # columns from the third onwards.
    result_row = row[2:]
```

## Development

```sh
cargo test
```

The test suite includes CLI checks and parity tests against a Python reference implementation when `python3` is available.
