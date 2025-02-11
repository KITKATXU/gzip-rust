# C to Rust Translation Tool for Gzip

This demonstrates the power of using a LLM (here, ChatGPT o1-preview) to safely and accurately translate code

## Progress Overview

Currently, this translation provides functionality for gzip compression levels -1 to -3 (fast_deflate), supporting both compression and decompression. 

We have migrated a robust suite of 69 tests, sourced entirely from the original C project. The translated code passes 64 out of 69 tests.

### Translation Process
1. **Globals Translation**:
    - The translation ensures platform independence and avoids mutable static variables for safety, instead moving globals to dependency-injected structs.

2. **Function Translation**:
    - Each function is translated individually, progressing up the function call graph.
    - **Challenges encountered**:
        - Sticking to C patterns (e.g., using `exit()` instead of returning from functions).
        - Duplicated globals and incorrect handling of conditional compilation flags.
        - Refusal to translate entire files or large functions, especially when the code is too complex.
        - Variable type mismatches in global structures.
        - Hallucinated function behavior, such as with `do_list`.
        - Misunderstandings of constants and compression methods.
        - Issues with variable borrowing and references in Rust, often due to direct C-to-Rust mapping.

    - **Strengths**:
        - The tool successfully utilizes external crates for platform independence, adapting Linux-specific code to cross-platform functionality.
        - It follows Rust paradigms when appropriate (e.g., replacing error codes with `Result` types), though this behavior needs improvement to fully migrate from C paradigms.
        - Providing both the full C file and the current function for translation helped reduce hallucinations.
        - With better prompting, the tool showed improvement in understanding more complex relationships and making the translation more idiomatic.

### Current Results

- **Current Tests Passed**: 64/69 Tests
- **Notable Issues**:
    - Some **bugs** were identified through manual testing, such as incorrect argument parsing ordering, which broke version and license flags.
    - **Rust auto-fixable warnings** were applied for code cleanliness, but the translated code still ignored `Result` handling in some cases.

### Common Errors
1. **Unspecified Generic Type**:
    - The tool often left generic types unspecified in places where they weren’t part of the function arguments. This was resolved with better prompting.

2. **Mutable Borrow on Immutably Borrowed Type**:
    - This led to borrow checker issues, but the model was able to generate an elegant fix when prompted to adjust the borrowing semantics.

## Outstanding Issues

- **Hallucinations and Incorrect Function Behavior**:  
  The model sometimes produces functions that don't match the intended logic of the original C code (e.g., `do_list` function). This requires providing more context or refining the model’s understanding.

- **Incomplete Handling of Constants**:  
  Some constants were misinterpreted or had incorrect mappings (e.g., compression method IDs and names). This may improve with more detailed prompting and context.

- **Variable Borrowing**:  
  The translation from C to Rust sometimes leads to borrow checker issues due to the incorrect handling of references, especially in the context of C-style arguments.

- **Limited Understanding of C Macros**:  
  The model struggles with handling C macros and conditional compilation flags, which may need special handling or manual intervention in certain cases.

- **Unsupported Negative Indexing**:
  Rust does not support negative indexing, and the model is unaware if an index is negative. This manifests in the code as direct conversions from signed to unsigned integers, causing out-of-bounds indexing. This requires the model to have a deep understanding of the C code, which is a complex task.

- **Omission of Functionality**:
  The model often truncates long functions, translating only parts of them, which results in incomplete translations. Manual segmentation is applied to ensure each part is fully translated, after which the segments are reassembled by the model.

## Next Steps and Plans

1. **Improve Prompting**:
    - Refine the prompting mechanism to improve the translation accuracy, particularly in cases of complex functions, macros, and constants.

2. **Address Hallucinations**:
    - Improve the model’s understanding of the original C code by providing more context, such as including more related function headers with the help of data flow graphs (DFG).

3. **Error Handling in Rust**:
    - Ensure that the Rust translation consistently handles errors with `Result` types and removes unsafe operations like `exit()` in favor of proper Rust error handling.

## Running the Project

### Build the Project
To build the project, run:
```bash
cargo build
```

### Run the Project
To run the translation tool:
```bash
cargo run
```

### Running Integration Tests

1. Install the Gzip dependencies:
    - Ensure you have the necessary dependencies for Gzip installed on your system.

2. Run the integration tests:
```bash
cargo test
```

## Conclusion

This project manually translating the gzip C project with the assistance of LLMs, highlighting the current limitations of LLMs in automated translation.

Since there has been no precedent for successfully translating C GNU packages, we not only fixed compilation errors but also manually resolved logical inconsistencies in translating C functionalities to Rust.
