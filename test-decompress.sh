#!/bin/bash

# Initialize counters for passed tests and total tests
passed=0
total=0

# Build the Rust project with the warning flag
RUSTFLAGS="-Awarnings" cargo build

# Create a temporary directory to store decompressed files
tmpdir=$(mktemp -d)
if [[ ! "$tmpdir" || ! -d "$tmpdir" ]]; then
  echo "Failed to create temporary directory."
  exit 1
fi

# Define a function to clean up the temporary directory upon script exit
cleanup() {
  rm -rf "$tmpdir"
}
trap cleanup EXIT

# Check if the custom gzip decompression tool exists and is executable
custom_gzip="/home/qingxiao/gzip-rust/target/debug/gzip"
if [[ ! -x "$custom_gzip" ]]; then
  echo "Custom gzip decompression tool not found or not executable: $custom_gzip"
  exit 1
fi

# Iterate over all regular files in the 'tests' directory
while IFS= read -r file; do
  # Increment the total test count
  total=$((total + 1))
  echo "Testing file: $file"

  # Compress the file using gzip with compression level 1 and keep the original file
  /home/qingxiao/my_gzip-c/bin/gzip -1 -k "$file"
  gzfile="${file}.gz"

  # Check if the compressed file was successfully created
  if [[ ! -f "$gzfile" ]]; then
    echo "Compression failed: Could not create gzip file $gzfile"
    continue
  fi

  # Define the path for the decompressed file using the custom gzip
  custom_decompressed="$tmpdir/$(basename "$file").custom"

  # Decompress the file using the custom gzip tool to the specified path
  "$custom_gzip" -d -c "$gzfile" > "$custom_decompressed"

  # Check if the custom decompression was successful
  if [[ $? -ne 0 ]]; then
    echo "Decompression failed: Custom gzip failed to decompress $gzfile."
    rm -f "$gzfile" "$custom_decompressed"
    continue
  fi

  # Compare the original file with the decompressed file to ensure they are identical
  if diff -q "$file" "$custom_decompressed" > /dev/null; then
    echo "Test passed."
    passed=$((passed + 1))
  else
    echo "Test failed: Decompressed file does not match the original."
  fi

  # Remove the compressed gzip file to keep the directory clean
  rm -f "$gzfile"

done < <(find tests -type f)

# Output the summary of test results
echo "Decompression Test Results: Passed $passed out of $total tests."

# # Set the script's exit status code
# if [[ $passed -ne $total ]]; then
#   echo "Some tests did not pass."
#   exit 1
# else
#   echo "All tests passed."
#   exit 0
# fi
