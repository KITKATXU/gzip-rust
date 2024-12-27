#!/bin/bash

passed=0
decom=0
total=0
decomtotal=0

compare_gzip_outputs() {
    # Capture all arguments passed to the function
    local args=("$@")
    file_name="${args[-1]}"

    # Create temporary files to store outputs
    GZIP_OUTPUT=$(mktemp)
    GZIP_OUTPUT_FILE=$(mktemp)
    CARGO_OUTPUT=$(mktemp)
    CARGO_OUTPUT_FILE=$(mktemp)

    # Run gzip with timeout
    timeout 5s gzip "${args[@]}" > "$GZIP_OUTPUT" 2>&1
    GZIP_STATUS=$?
    mv "$file_name.gz" "$GZIP_OUTPUT_FILE" > /dev/null 2>&1

    # Run cargo run with timeout
    cargo build > /dev/null 2>&1
    timeout 5s ./target/debug/gzip "${args[@]}" > "$CARGO_OUTPUT" 2>&1
    CARGO_STATUS=$?
    mv "$file_name.gz" "$CARGO_OUTPUT_FILE" > /dev/null 2>&1

    # Check for timeout or compare outputs
    if [ $GZIP_STATUS -eq 124 ] || [ $CARGO_STATUS -eq 124 ]; then
        echo "Test failed: Timeout occurred"
        mv "$GZIP_OUTPUT" "target/gzip_console_output.txt"
        mv "$CARGO_OUTPUT" "target/cargo_console_output.txt"
    elif diff -u "$GZIP_OUTPUT" "$CARGO_OUTPUT" && diff -u "$GZIP_OUTPUT_FILE" "$CARGO_OUTPUT_FILE"; then
        echo "Test passed."
        ((passed++))
    else
        echo "Test failed."
        # cat $CARGO_OUTPUT
        mv "$GZIP_OUTPUT" "target/gzip_console_output.txt"
        mv "$CARGO_OUTPUT" "target/cargo_console_output.txt"
        mv "$GZIP_OUTPUT_FILE" "target/gzip_output.gz"
        mv "$CARGO_OUTPUT_FILE" "target/cargo_output.gz"
    fi

    # Clean up temporary files
    rm "$GZIP_OUTPUT" "$GZIP_OUTPUT_FILE" "$CARGO_OUTPUT" "$CARGO_OUTPUT_FILE" > /dev/null 2>&1
    ((total++))
}

compare_gzip_outputs_no_file() {
    # Capture all arguments passed to the function
    local args=("$@")

    # Create temporary files to store outputs
    GZIP_OUTPUT=$(mktemp)
    CARGO_OUTPUT=$(mktemp)

    # Run gzip with the provided arguments and capture stdout and stderr
    gzip "${args[@]}" < /dev/null > "$GZIP_OUTPUT" 2>&1

    # Run cargo run with the provided arguments and capture stdout and stderr
    cargo build > /dev/null 2>&1

    ./target/debug/gzip "${args[@]}" > "$CARGO_OUTPUT" 2>&1

    # Compare the outputs
    if diff -u "$GZIP_OUTPUT" "$CARGO_OUTPUT"; then
        echo "Test passed."
        ((passed++))
    else
        echo "Test failed. Wrote failed files to target"
        mv "$GZIP_OUTPUT" "target/gzip_console_output.txt"
        mv "$CARGO_OUTPUT" "target/cargo_console_output.txt"
    fi

    # Clean up temporary files
    rm "$GZIP_OUTPUT" "$CARGO_OUTPUT" > /dev/null 2>&1
    ((total++))
}

echo "Testing no-arg output"
compare_gzip_outputs_no_file " "

echo "Testing nonexistant"
compare_gzip_outputs -k -1 test.txt

echo "Testing already existing output"
touch tests/test-word.txt.gz
# compare_gzip_outputs -k -1 tests/test-word.txt

echo "Testing forced overwrite"
compare_gzip_outputs -k -f -1 tests/test-word.txt

echo "Testing delete"
echo "test" > tests/test-temp.txt
./target/debug/gzip -f -1 tests/test-temp.txt
echo "Testing file is deleted"
if [ -f tests/test-temp.txt ]; then
  echo "Test failed. File not deleted"
else
  echo "Test passed."
  ((passed++))
fi
((total++))

echo "Testing help menu"
compare_gzip_outputs_no_file -h

echo "Testing empty bits operand"
compare_gzip_outputs_no_file -b

echo "Testing incorrect bits operand"
compare_gzip_outputs_no_file -b test

echo "Testing bits operand"
compare_gzip_outputs -k -1 -b 3 tests/test-word.txt

echo "Testing compression level 1"
compare_gzip_outputs -k -1 tests/test-word.txt

echo "Testing compression level 2"
compare_gzip_outputs -k -2 tests/test-word.txt

echo "Testing compression level 3"
compare_gzip_outputs -k -3 tests/test-word.txt

echo "Testing ascii mode"
compare_gzip_outputs -k -a -1 tests/test-word.txt

echo "Testing stdout mode"
compare_gzip_outputs -k -c -1 tests/test-word.txt

echo "Testing quiet mode"
compare_gzip_outputs -k -q -1 tests/test-word.txt

echo "Testing no name mode"
compare_gzip_outputs -k -n -1 tests/test-word.txt

echo "Testing large arg combinations"
compare_gzip_outputs -k -a -b 3 -q -n -1 tests/test-word.txt

echo "Testing recursive"
compare_gzip_outputs -r -k -1 tests/testing

echo "Testing combined options (-1c)"
compare_gzip_outputs -k -1c tests/test-word.txt

echo "Testing suffix option (-S)"
compare_gzip_outputs -k -S .gzip tests/test-word.txt

echo "Testing stdin"
# Create reference gzip output
gzip -1 < tests/test-word.txt > tests/output.gz
# Create your implementation's output
./target/debug/gzip -1 -f < tests/test-word.txt > tests/test-word.txt.gz
# Decompress both files and compare their contents instead of comparing gz files directly
gzip -dc tests/output.gz > tests/output.txt
gzip -dc tests/test-word.txt.gz > tests/test-word.decoded.txt
if diff -u "tests/output.txt" "tests/test-word.decoded.txt" >/dev/null 2>&1; then
    echo "Test passed."
    ((passed++))
else
    echo "Test failed."
    cp tests/output.gz target/test-stdin.gz
    cp tests/test-word.txt.gz target/test-target.gz
fi
((total++))
# rm tests/testing/*.gz
rm tests/*.gz
rm -f tests/output.txt tests/test-word.decoded.txt

echo "Testing version"
compare_gzip_outputs_no_file -L

# Testing reproducible compression
echo "Testing reproducible compression..."
TEST_INPUT=$(mktemp)
echo "test data" > "$TEST_INPUT"

# First compression
./target/debug/gzip -k -1 -c "$TEST_INPUT" > comp1.gz
sleep 1
# Second compression
./target/debug/gzip -k -1 -c "$TEST_INPUT" > comp2.gz

if diff -u comp1.gz comp2.gz > /dev/null 2>&1; then
    echo "Compression reproducibility test passed."
    ((passed++))
else
    echo "Compression reproducibility test failed."
    mv comp1.gz target/comp1.gz
    mv comp2.gz target/comp2.gz
fi
((total++))
rm -f comp1.gz comp2.gz "$TEST_INPUT"

# Create test file for GZIP env variable tests
# echo "test data" > tests/test-word.txt
# gzip -k tests/test-word.txt || exit 1

# echo "Testing test files"
# for file in tests/*; do
#   echo "Testing $file"
#   compare_gzip_outputs -k -f -1 "$file"
# done

# Replace the test files loop with size-based compression tests
echo "Testing compression of different file sizes"

# # Generate test string
# in_str=0123456789abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ_-+=%
# for i in 0 1 2 3 4 5 6 7 8 9 a; do 
#     in_str="$in_str$in_str"
# done

# # Test specific sizes
# sizes="0 1 2 3 4 32831 32832 32833 131071 131072 131073"
# for size in $sizes; do
#     echo "Testing compression for size: $size"
    
#     # Create input file of specific size
#     TEST_INPUT=$(mktemp)
#     printf %$size.${size}s "$in_str" > "$TEST_INPUT"
    
#     # Test compression using the comparison function
#     compare_gzip_outputs -k -f -1 "$TEST_INPUT"
    
#     # Cleanup
#     rm -f "$TEST_INPUT"
# done
# Generate test string
in_str="0123456789abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ_-+=%"
for i in 0 1 2 3 4 5 6 7 8 9 a; do 
    in_str="${in_str}${in_str}"
done

# Define test sizes
sizes=("0" "1" "2" "3" "4" "32831" "32832" "32833" "131071" "131072" "131073")

for size in "${sizes[@]}"; do
    echo "Testing size: $size"
    
    # Create input file of specific size
    TEST_INPUT=$(mktemp)
    printf "%.${size}s" "$in_str" > "$TEST_INPUT"
    
    # Compress with Rust implementation
    ./target/debug/gzip -c -1 "$TEST_INPUT" > "$TEST_INPUT-temp.gz"
    
    # Create temporary file for decompressed output
    DECOMPRESSED_OUTPUT=$(mktemp)
    
    # Decompress using original C implementation
    /home/qingxiao/my_gzip-c/bin/gzip -d -c "$TEST_INPUT-temp.gz" > "$DECOMPRESSED_OUTPUT" 2>/dev/null
    
    # Compare the files
    if diff -u "$TEST_INPUT" "$DECOMPRESSED_OUTPUT" >/dev/null 2>&1; then
        echo "Test passed for size $size"
        ((passed++))
    else
        echo "Test failed for size $size"
        mkdir -p target/failed_tests
        cp "$TEST_INPUT" "target/failed_tests/original_file_${size}.txt"
        cp "$DECOMPRESSED_OUTPUT" "target/failed_tests/decompressed_file_${size}.txt"
        cp "$TEST_INPUT-temp.gz" "target/failed_tests/compressed_file_${size}.gz"
    fi
    ((total++))
    
    # Cleanup
    rm -f "$TEST_INPUT-temp.gz" "$DECOMPRESSED_OUTPUT" "$TEST_INPUT"
done


# Helper function for decompression tests
compare_decompression() {
    # Capture all arguments passed to the function
    local args=("$@")
    local GZIP_OUTPUT=$(mktemp)
    local CARGO_OUTPUT=$(mktemp)
    
    # Save a copy of the gz file if it exists
    local gz_file="${args[-1]}"
    local gz_backup=""
    if [ -f "$gz_file" ]; then
        gz_backup=$(mktemp)
        cp "$gz_file" "$gz_backup"
    fi

    # Run standard gzip decompression with timeout - add -f flag
    timeout 5s gzip  "${args[@]}" > "$GZIP_OUTPUT" 2>&1
    GZIP_STATUS=$?
    

    # Restore gz file for our implementation
    if [ -n "$gz_backup" ]; then
        cp "$gz_backup" "$gz_file"
    fi

    # Run our implementation with timeout
    timeout 5s ./target/debug/gzip  "${args[@]}" > "$CARGO_OUTPUT" 2>&1
    CARGO_STATUS=$?

    # Check for timeout or compare outputs
    if [ $GZIP_STATUS -eq 124 ] || [ $CARGO_STATUS -eq 124 ]; then
        echo "Test failed: Timeout occurred"
        mv "$GZIP_OUTPUT" "target/gzip_decomp_output.txt"
        mv "$CARGO_OUTPUT" "target/cargo_decomp_output.txt"
    elif diff -u "$GZIP_OUTPUT" "$CARGO_OUTPUT"; then
        echo "Test passed."
        ((decom++))
    else
        echo "Test failed."
        mv "$GZIP_OUTPUT" "target/gzip_decomp_output.txt"
        mv "$CARGO_OUTPUT" "target/cargo_decomp_output.txt"
    fi

    # Clean up temporary files
    rm -f "$GZIP_OUTPUT" "$CARGO_OUTPUT"
    if [ -n "$gz_backup" ]; then
        rm -f "$gz_backup"
    fi
    ((decomtotal++))
}
compare_gzip_decompress_outputs() {
    # Get the last argument (input file)
    local input_file="${@: -1}"
    # Get all arguments except the last one (options)
    local options=("${@:1:$#-1}")

    # Ensure target directory exists
    mkdir -p target

    # Create temporary directory to avoid filename conflicts
    local temp_dir
    temp_dir=$(mktemp -d)
    if [ ! -d "$temp_dir" ]; then
        echo "Failed to create temporary directory"
        return 1
    fi

    # Define temporary file paths
    local compressed_file="$temp_dir/compressed.gz"
    local system_decompressed="$temp_dir/system_decompressed"
    local cargo_decompressed="$temp_dir/cargo_decompressed"

    # Create temporary files to store console output
    local GZIP_OUTPUT
    local CARGO_OUTPUT
    GZIP_OUTPUT=$(mktemp)
    CARGO_OUTPUT=$(mktemp)

    # Use system gzip to compress input file
    timeout 5s gzip -c -r "$input_file" > "$compressed_file" 
    local GZIP_COMPRESS_STATUS=$?

    # Check if compression was successful
    if [ $GZIP_COMPRESS_STATUS -ne 0 ]; then
        echo "Test failed: System gzip compression failed"
        mv "$GZIP_OUTPUT" "target/gzip_compress_console_output.txt"
        rm -rf "$temp_dir" "$GZIP_OUTPUT" "$CARGO_OUTPUT"
        return 1
    fi

    # Ensure cargo project is built
    cargo build > /dev/null 2>&1
    if [ $? -ne 0 ]; then
        echo "Test failed: cargo build failed"
        rm -rf "$temp_dir" "$GZIP_OUTPUT" "$CARGO_OUTPUT"
        return 1
    fi

    # Use system gzip to decompress the compressed file with options
    timeout 5s gzip "${options[@]}" -c "$compressed_file" > "$system_decompressed" 2> "$GZIP_OUTPUT"
    local GZIP_DECOMPRESS_STATUS=$?

    # Use cargo-built gzip to decompress the compressed file with options
    timeout 5s ./target/debug/gzip "${options[@]}" -c "$compressed_file" > "$cargo_decompressed" 2> "$CARGO_OUTPUT"
    local CARGO_DECOMPRESS_STATUS=$?

    # Check for decompression timeout
    if [ $GZIP_DECOMPRESS_STATUS -eq 124 ] || [ $CARGO_DECOMPRESS_STATUS -eq 124 ]; then
        echo "Test failed: Timeout occurred during decompression"
        mv "$GZIP_OUTPUT" "target/gzip_decompress_console_output.txt"
        mv "$CARGO_OUTPUT" "target/cargo_decompress_console_output.txt"
        rm -rf "$temp_dir" "$GZIP_OUTPUT" "$CARGO_OUTPUT"
        return 1
    fi

    # Compare decompressed files
    local DECOMPRESS_DIFF=0
    diff -u "$system_decompressed" "$cargo_decompressed" > /dev/null 2>&1
    if [ $? -ne 0 ]; then
        DECOMPRESS_DIFF=1
    fi

    # Evaluate test results
    if [ $DECOMPRESS_DIFF -eq 0 ]; then
        echo "Test passed."
        ((decom++))
    else
        echo "Test failed."
        # Save console output
        mv "$GZIP_OUTPUT" "target/gzip_decompress_console_output.txt"
        mv "$CARGO_OUTPUT" "target/cargo_decompress_console_output.txt"
        # Save decompressed files
        mv "$system_decompressed" "target/system_decompressed_output"
        mv "$cargo_decompressed" "target/cargo_decompressed_output"
    fi

    # Increment total test count
    ((decomtotal++))

    # Clean up temporary files and directories
    rm -rf "$temp_dir" "$GZIP_OUTPUT" "$CARGO_OUTPUT"
}


# Test help menu for decompression
echo "Testing decompression help menu"
compare_decompression -d -h

echo "Testing decompression no-arg output"
compare_decompression -d

echo "Testing decompression nonexistant"
compare_decompression -d test.txt.gz

echo "Testing decompression already existing output"
touch tests/test-word.txt.gz
# compare_gzip_outputs -k -1 tests/test-word.txt

# Test quiet mode
echo "Testing quiet mode decompression"
compare_gzip_decompress_outputs -d -q tests/test-word.txt
rm -f tests/test-word.txt.gz

# Test force overwrite
echo "Testing forced overwrite for decompression"
compare_gzip_decompress_outputs -d -f tests/test-word.txt
rm -f tests/test-word.txt.gz 

# # Test stdout mode
echo "Testing stdout mode for decompression"
compare_gzip_decompress_outputs -d -c tests/test-word.txt
rm -f tests/test-word.txt.gz

# # Test recursive decompression
echo "Testing recursive decompression"
mkdir -p tests/testing_decomp
cp tests/test-word.txt tests/testing_decomp/
# gzip -k -1 tests/testing_decomp/test-word.txt
compare_gzip_decompress_outputs -d -r tests/testing_decomp
rm -rf tests/testing_decomp

# # Test no name mode
echo "Testing no name mode for decompression"
compare_gzip_decompress_outputs -d -n tests/test-word.txt
rm -f tests/test-word.txt.gz

# # Test combination of parameters
echo "Testing parameter combinations for decompression"
compare_gzip_decompress_outputs -d -f -n -q tests/test-word.txt
rm -f tests/test-word.txt.gz

# # Test non-integer parameter
echo "Testing decompression of non-integer parameter"
compare_gzip_decompress_outputs -d -b tests/test-word.txt
rm -f tests/test-word.txt.gz

# # Level 1
echo "Testing decompression of level 1 compressed file"
compare_gzip_decompress_outputs -d -1 tests/test-word.txt
rm -f tests/test-word.txt.gz

# # Level 2
echo "Testing decompression of level 2 compressed file"
compare_gzip_decompress_outputs -d -2 tests/test-word.txt
rm -f tests/test-word.txt.gz

# # Level 3
echo "Testing decompression of level 3 compressed file"
compare_gzip_decompress_outputs -d -3 tests/test-word.txt
rm -f tests/test-word.txt.gz

echo "Testing combined options (-cdf)"
compare_gzip_decompress_outputs -cdf tests/test-word.txt

echo "Testing suffix option (-S)"
compare_gzip_decompress_outputs -d -S .gzip tests/test-word.txt


echo "Testing decompression files"

# Create test file for GZIP env variable tests
echo "test data" > tests/test-word.txt
gzip -k tests/test-word.txt || exit 1

# Test one valid GZIP environment variable option
echo "Testing GZIP environment variable with valid option"
GZIP_OUTPUT=$(mktemp)
CARGO_OUTPUT=$(mktemp)

GZIP="-1" gzip -d < tests/test-word.txt.gz > "$GZIP_OUTPUT" 2>&1
GZIP="-1" ./target/debug/gzip -d < tests/test-word.txt.gz > "$CARGO_OUTPUT" 2>&1

if diff -u "$GZIP_OUTPUT" "$CARGO_OUTPUT"; then
    echo "Test passed."
    ((decom++))
else
    echo "Test failed."
    mv "$GZIP_OUTPUT" "target/gzip_env_good_output.txt"
    mv "$CARGO_OUTPUT" "target/cargo_env_good_output.txt"
fi
rm -f "$GZIP_OUTPUT" "$CARGO_OUTPUT"
((decomtotal++))

# Test one invalid GZIP environment variable option
echo "Testing GZIP environment variable with invalid option"
GZIP_OUTPUT=$(mktemp)
CARGO_OUTPUT=$(mktemp)

GZIP="--stdout" gzip -d < tests/test-word.txt.gz > "$GZIP_OUTPUT" 2>&1
GZIP="--stdout" ./target/debug/gzip -d < tests/test-word.txt.gz > "$CARGO_OUTPUT" 2>&1

if diff -u "$GZIP_OUTPUT" "$CARGO_OUTPUT"; then
    echo "Test passed."
    ((decom++))
else
    echo "Test failed."
    mv "$GZIP_OUTPUT" "target/gzip_env_bad_output.txt"
    mv "$CARGO_OUTPUT" "target/cargo_env_bad_output.txt"
fi
rm -f "$GZIP_OUTPUT" "$CARGO_OUTPUT" tests/test-word.txt.gz
((decomtotal++))

# for file in tests/*; do
#   echo "Testing decompression of $file"
#   # Compress with standard gzip (using my_gzip-c implementation for consistency)
#   /home/qingxiao/my_gzip-c/bin/gzip -k -1 "$file"
#   mv "$file.gz" "$file-temp.gz"
  
#   # Create temporary file for decompressed output
#   DECOMPRESSED_OUTPUT=$(mktemp)
  
#   # Decompress using our implementation
#   ./target/debug/gzip -d -c "$file-temp.gz" > "$DECOMPRESSED_OUTPUT" 2>/dev/null
  
#   # Compare the files
#   if diff -u "$file" "$DECOMPRESSED_OUTPUT" >/dev/null 2>&1; then
#     echo "Test passed."
#     ((decom++))
#   else
#     echo "Test failed."
#     cp "$file" "target/original_file.txt"
#     cp "$DECOMPRESSED_OUTPUT" "target/decompressed_file.txt"
#     cp "$file-temp.gz" "target/compressed_file.gz"
#   fi
#   ((decomtotal++))
  
#   # Cleanup
#   rm -f "$file-temp.gz" "$DECOMPRESSED_OUTPUT"
# done

# Replace the final testing loop with size-based tests
echo "Testing decompression of different file sizes"

# Generate test string
in_str=0123456789abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ_-+=%
for i in 0 1 2 3 4 5 6 7 8 9 a; do 
    in_str="$in_str$in_str"
done

# Test specific sizes
sizes="0 1 2 3 4 32831 32832 32833 131071 131072 131073"
for size in $sizes; do
    echo "Testing size: $size"
    
    # Create input file of specific size
    TEST_INPUT=$(mktemp)
    printf %$size.${size}s "$in_str" > "$TEST_INPUT"
    
    # Compress with reference implementation
    /home/qingxiao/my_gzip-c/bin/gzip -k -1 "$TEST_INPUT"
    mv "$TEST_INPUT.gz" "$TEST_INPUT-temp.gz"
    
    # Create temporary file for decompressed output
    DECOMPRESSED_OUTPUT=$(mktemp)
    
    # Decompress using our implementation
    ./target/debug/gzip -d -c "$TEST_INPUT-temp.gz" > "$DECOMPRESSED_OUTPUT" 2>/dev/null
    
    # Compare the files
    if diff -u "$TEST_INPUT" "$DECOMPRESSED_OUTPUT" >/dev/null 2>&1; then
        echo "Test passed for size $size"
        ((decom++))
    else
        echo "Test failed for size $size"
        cp "$TEST_INPUT" "target/original_file_${size}.txt"
        cp "$DECOMPRESSED_OUTPUT" "target/decompressed_file_${size}.txt"
        cp "$TEST_INPUT-temp.gz" "target/compressed_file_${size}.gz"
    fi
    ((decomtotal++))
    
    # Cleanup
    rm -f "$TEST_INPUT-temp.gz" "$DECOMPRESSED_OUTPUT" "$TEST_INPUT"
done


echo "Testing huft decompression cases"

# Test Case 1: Decompressing hufts-segv.gz
echo "Testing hufts-segv.gz decompression"
GZIP_OUTPUT=$(mktemp)
CARGO_OUTPUT=$(mktemp)

gzip -d < hufts-segv.gz > "$GZIP_OUTPUT" 2>&1
GZIP_STATUS=$?
./target/debug/gzip -d < hufts-segv.gz > "$CARGO_OUTPUT" 2>&1
CARGO_STATUS=$?

if [ "$GZIP_STATUS" -eq "$CARGO_STATUS" ] && diff -u "$GZIP_OUTPUT" "$CARGO_OUTPUT"; then
    echo "Test passed."
    ((decom++))
else
    echo "Test failed."
    mv "$GZIP_OUTPUT" "target/gzip_huft_segv_output.txt"
    mv "$CARGO_OUTPUT" "target/cargo_huft_segv_output.txt"
fi
rm -f "$GZIP_OUTPUT" "$CARGO_OUTPUT"
((decomtotal++))

# Test Case 2: Decompressing bug33501
echo "Testing bug33501 decompression"
GZIP_OUTPUT=$(mktemp)
CARGO_OUTPUT=$(mktemp)
TEST_FILE=$(mktemp)

# Create the invalid gzip file
printf '\037\213\010\000\060\060\060\060\060\060\144\000\000\000' > "$TEST_FILE"

gzip -d < "$TEST_FILE" > "$GZIP_OUTPUT" 2>&1
GZIP_STATUS=$?
./target/debug/gzip -d < "$TEST_FILE" > "$CARGO_OUTPUT" 2>&1
CARGO_STATUS=$?

if [ "$GZIP_STATUS" -eq "$CARGO_STATUS" ] && diff -u "$GZIP_OUTPUT" "$CARGO_OUTPUT"; then
    echo "Test passed."
    ((decom++))
else
    echo "Test failed."
    mv "$GZIP_OUTPUT" "target/gzip_bug33501_output.txt"
    mv "$CARGO_OUTPUT" "target/cargo_bug33501_output.txt"
fi
rm -f "$GZIP_OUTPUT" "$CARGO_OUTPUT" "$TEST_FILE"
((decomtotal++))

# Test mixed data cases
echo "Testing mixed data cases..."

# Test 1.1: Pure uncompressed data
echo "Test 1.1: Pure uncompressed data"
GZIP_OUTPUT=$(mktemp)
CARGO_OUTPUT=$(mktemp)

(echo xxx; echo yyy) > in
gzip -c -d -f < in > "$GZIP_OUTPUT" 2>&1
./target/debug/gzip -c -d -f < in > "$CARGO_OUTPUT" 2>&1

if diff -u "$GZIP_OUTPUT" "$CARGO_OUTPUT"; then
    echo "PASS: Pure uncompressed data test"
    ((decom++))
else
    echo "FAIL: Pure uncompressed data test"
    mv "$GZIP_OUTPUT" "target/gzip_pure_uncomp_output.txt"
    mv "$CARGO_OUTPUT" "target/cargo_pure_uncomp_output.txt"
fi
rm -f "$GZIP_OUTPUT" "$CARGO_OUTPUT" in
((decomtotal++))

# Test 1.2: Compressed followed by uncompressed
echo "Test 1.2: Compressed followed by uncompressed"
GZIP_OUTPUT=$(mktemp)
CARGO_OUTPUT=$(mktemp)

(echo xxx | gzip; echo yyy) > in
gzip -c -d -f < in > "$GZIP_OUTPUT" 2>&1
./target/debug/gzip -c -d -f < in > "$CARGO_OUTPUT" 2>&1

if diff -u "$GZIP_OUTPUT" "$CARGO_OUTPUT"; then
    echo "PASS: Compressed + uncompressed test"
    ((decom++))
else
    echo "FAIL: Compressed + uncompressed test"
    mv "$GZIP_OUTPUT" "target/gzip_comp_uncomp_output.txt"
    mv "$CARGO_OUTPUT" "target/cargo_comp_uncomp_output.txt"
fi
rm -f "$GZIP_OUTPUT" "$CARGO_OUTPUT" in
((decomtotal++))

# Test 1.3: Double compressed data
echo "Test 1.3: Double compressed data"
GZIP_OUTPUT=$(mktemp)
CARGO_OUTPUT=$(mktemp)

(echo xxx | gzip; echo yyy | gzip) > in
gzip -c -d -f < in > "$GZIP_OUTPUT" 2>&1
./target/debug/gzip -c -d -f < in > "$CARGO_OUTPUT" 2>&1

if diff -u "$GZIP_OUTPUT" "$CARGO_OUTPUT"; then
    echo "PASS: Double compressed test"
    ((decom++))
else
    echo "FAIL: Double compressed test"
    mv "$GZIP_OUTPUT" "target/gzip_double_comp_output.txt"
    mv "$CARGO_OUTPUT" "target/cargo_double_comp_output.txt"
fi
rm -f "$GZIP_OUTPUT" "$CARGO_OUTPUT" in
((decomtotal++))

# Testing reproducible decompression
echo "Testing reproducible decompression..."
TEST_INPUT=$(mktemp)
echo "test data" > "$TEST_INPUT"
gzip -c "$TEST_INPUT" > test.gz

# First decompression
./target/debug/gzip -d -c test.gz > decomp1
sleep 1
# Second decompression
./target/debug/gzip -d -c test.gz > decomp2

if diff -u decomp1 decomp2 > /dev/null 2>&1; then
    echo "Decompression reproducibility test passed."
    ((decom++))
else
    echo "Decompression reproducibility test failed."
    mv decomp1 target/decomp1
    mv decomp2 target/decomp2
    mv test.gz target/test.gz
fi
((decomtotal++))
rm -f decomp1 decomp2 test.gz "$TEST_INPUT"

echo "Compression Tests passed: $passed out of $total"
echo "Decompression Tests passed: $decom out of $decomtotal"