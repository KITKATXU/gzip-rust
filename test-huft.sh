#!/bin/bash

# Initialize counters
total_tests=0
passed_tests=0

# Build the Rust project with the warning flag
RUSTFLAGS="-Awarnings" cargo build

# Iterate over all files in the tests folder
for test_file in tests/*; do
    # Ensure it's a regular file
    if [[ -f "$test_file" ]]; then
        total_tests=$((total_tests + 1))
        
        # Copy the original file for testing
        cp "$test_file" testc.txt
        gzip -1 testc.txt

        # Decompress and compare output
        gzip -d -p testc.txt.gz > output_gzip_d.txt
        rm -f testc.txt  

        # Test with the target executable
        cp "$test_file" testr.txt
        gzip -1 testr.txt

        target/debug/gzip -H testr.txt.gz > output_target_debug.txt
        rm -f testr.txt  

        # Compare the outputs
        if diff output_gzip_d.txt output_target_debug.txt > /dev/null; then
            echo "$test_file: Huffman tree test passed"
            passed_tests=$((passed_tests + 1))
        else
            echo "$test_file: Huffman tree test failed"
        fi

        # Clean up temporary files
        rm output_gzip_d.txt output_target_debug.txt
    fi
done

# Output the test results
echo "$passed_tests/$total_tests tests passed"
