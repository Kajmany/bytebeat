#!/usr/bin/env bash
# Script to check C compatibility of bytebeat songs in library.csv
#
# Attempts to compile each song from CSV, reports failures and can remove from the library.
# Originally slopped and not the prettiest parsing/editing, but I removed some nonsense like making temp files for each compilation
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
CSV_FILE="${SCRIPT_DIR}/library.csv"
OUTPUT_FILE="${SCRIPT_DIR}/c_compat_failures.txt"

# Clear the output file
> "$OUTPUT_FILE"

echo "Checking C compatibility of songs in ${CSV_FILE}..."
echo "Results will be written to ${OUTPUT_FILE}"
echo ""

line_num=0
fail_count=0
success_count=0
failed_lines=()

# Read CSV line by line (skip header)
while IFS= read -r line; do
    ((line_num++)) || true
    
    # Skip header line
    if [[ $line_num -eq 1 ]]; then
        continue
    fi
    
    # Since every field is quoted and this is easier than treating it like an actual CSV
    # I tried a Regex. There's things with quotes going on inside that make it not worthwhile.
    code=$(echo "$line" | rev | cut -d'"' -f2 | rev)
    
    # Die if we couldn't extract code
    if [[ -z "$code" ]]; then
        echo "Line $line_num: Could not extract code"
        exit
    fi
    
    # Also extract name for better reporting (2nd field)
    name=$(echo "$line" | cut -d'"' -f4)
    author=$(echo "$line" | cut -d'"' -f2)
    
    # Try to compile by piping C source directly to compiler
    if ! error_output=$(cc -std=c99 -Wall -Wextra -fsyntax-only -x c - 2>&1 <<EOF
#include <stdint.h>

uint8_t song(int t) {
    return (${code});
}

int main(void) {
    return 0;
}
EOF
); then
        ((fail_count++)) || true
        failed_lines+=("$line_num")
        
        echo "=== Line $line_num ===" >> "$OUTPUT_FILE"
        echo "Author: $author" >> "$OUTPUT_FILE"
        echo "Name: $name" >> "$OUTPUT_FILE"
        echo "Code: $code" >> "$OUTPUT_FILE"
        echo "" >> "$OUTPUT_FILE"
        echo "Compiler errors:" >> "$OUTPUT_FILE"
        echo "$error_output" >> "$OUTPUT_FILE"
        echo "" >> "$OUTPUT_FILE"
        echo "---" >> "$OUTPUT_FILE"
        echo "" >> "$OUTPUT_FILE"
        
        echo "FAIL: Line $line_num - ${author:-(unknown)}: ${name:-(untitled)}"
    else
        ((success_count++)) || true
    fi
    
done < "$CSV_FILE"

echo ""
echo "========================================="
echo "Summary:"
echo "  Total songs: $((success_count + fail_count))"
echo "  Successful:  $success_count"
echo "  Failed:      $fail_count"
echo "========================================="
echo ""
echo "Failure details written to: $OUTPUT_FILE"

# Offer to remove failed lines if there are any
if [[ ${#failed_lines[@]} -gt 0 ]]; then
    echo ""
    echo "Would you like to remove the ${#failed_lines[@]} non-C-compatible songs from library.csv?"
    echo "A backup will be created at library.csv.bak"
    read -rp "Remove failed songs? [y/N]: " response
    
    if [[ "$response" =~ ^[Yy]$ ]]; then
        # Create backup
        cp "$CSV_FILE" "${CSV_FILE}.bak"
        echo "Backup created: ${CSV_FILE}.bak"
        
        # Build sed expression to delete failed lines
        sed_expr=""
        for ln in "${failed_lines[@]}"; do
            sed_expr+="${ln}d;"
        done
        
        # Remove the lines
        sed -i "$sed_expr" "$CSV_FILE"
        
        echo "Removed ${#failed_lines[@]} lines from library.csv"
        echo "Lines removed: ${failed_lines[*]}"
    else
        echo "No changes made to library.csv"
    fi
fi
