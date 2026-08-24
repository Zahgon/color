// Emits the exact set of code points for which unicode.IsPrint is false,
// as compact ranges, for embedding in the Rust port.
package main

import (
	"fmt"
	"unicode"
	"unicode/utf8"
)

func main() {
	type rng struct{ lo, hi rune }
	var ranges []rng
	var start rune = -1

	for r := rune(0); r <= utf8.MaxRune; r++ {
		if r >= 0xD800 && r <= 0xDFFF {
			// Surrogates are not representable as a Rust `char`; skip so they
			// do not fragment the ranges.
			continue
		}
		np := !unicode.IsPrint(r)
		if np && start < 0 {
			start = r
		} else if !np && start >= 0 {
			ranges = append(ranges, rng{start, r - 1})
			start = -1
		}
	}
	if start >= 0 {
		ranges = append(ranges, rng{start, utf8.MaxRune})
	}

	fmt.Printf("// %d ranges\n", len(ranges))
	for _, r := range ranges {
		fmt.Printf("(0x%04X, 0x%04X),\n", r.lo, r.hi)
	}
}
