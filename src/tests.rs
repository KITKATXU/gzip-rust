use std::fs::{self, File};
use std::io::Write;
use std::process::Command;
use tempfile::NamedTempFile;
use std::path::Path;
use std::process::Stdio; 

// Helper function to compare outputs between system gzip and our implementation
fn compare_gzip_outputs(args: &[&str], input_file: Option<&str>) -> bool {
    // Clean up any existing .gz files before test
    if let Some(file) = input_file {
        let gz_path = format!("{}.gz", file);
        fs::remove_file(&gz_path).ok();
    }

    // Build the project first
    Command::new("cargo")
        .arg("build")
        .output()
        .expect("Failed to build project");

    // Create temp files for outputs
    let gzip_output = NamedTempFile::new().unwrap();
    let cargo_output = NamedTempFile::new().unwrap();

    // Run system gzip
    let gzip_status = Command::new("gzip")
        .args(args)
        .output()
        .expect("Failed to execute gzip");

    // Clean up any .gz files before running our implementation
    if let Some(file) = input_file {
        let gz_path = format!("{}.gz", file);
        fs::remove_file(&gz_path).ok();
    }

    // Run our implementation
    let cargo_status = Command::new("./target/debug/gzip")
        .args(args)
        .output()
        .expect("Failed to execute our gzip");

    // Compare outputs
    let stdout_matches = gzip_status.stdout == cargo_status.stdout;
    let stderr_matches = gzip_status.stderr == cargo_status.stderr;
    
    if !stdout_matches || !stderr_matches {
        println!("\nOutput comparison failed!");
        println!("System gzip stdout: {:?}", String::from_utf8_lossy(&gzip_status.stdout));
        println!("Our gzip stdout: {:?}", String::from_utf8_lossy(&cargo_status.stdout));
        println!("System gzip stderr: {:?}", String::from_utf8_lossy(&gzip_status.stderr));
        println!("Our gzip stderr: {:?}", String::from_utf8_lossy(&cargo_status.stderr));
    }

    // Clean up any remaining .gz files after test
    if let Some(file) = input_file {
        let gz_path = format!("{}.gz", file);
        fs::remove_file(&gz_path).ok();
    }
    
    stdout_matches && stderr_matches
}

// Helper function to compare decompression outputs between system gzip and our implementation
fn compare_decompress_gzip_outputs(args: &[&str], gz_file: &str) -> bool {
    // Create a copy of the gz file for our implementation
    let our_gz = format!("{}.copy.gz", gz_file.strip_suffix(".gz").unwrap_or(gz_file));
    fs::copy(gz_file, &our_gz).expect("Failed to copy gz file");

    // Run system gzip
    let gzip_status = Command::new("gzip")
        .args(args)
        .arg(gz_file)
        .output()
        .expect("Failed to execute gzip");

    // Run our implementation
    let cargo_status = Command::new("./target/debug/gzip")
        .args(args)
        .arg(&our_gz)
        .output()
        .expect("Failed to execute our gzip");

    // Clean up the copy
    fs::remove_file(&our_gz).ok();

    // Compare outputs
    let stdout_matches = gzip_status.stdout == cargo_status.stdout;
    let stderr_matches = gzip_status.stderr == cargo_status.stderr;
    
    if !stdout_matches || !stderr_matches {
        println!("\nOutput comparison failed!");
        println!("System gzip stdout: {:?}", String::from_utf8_lossy(&gzip_status.stdout));
        println!("Our gzip stdout: {:?}", String::from_utf8_lossy(&cargo_status.stdout));
        println!("System gzip stderr: {:?}", String::from_utf8_lossy(&gzip_status.stderr));
        println!("Our gzip stderr: {:?}", String::from_utf8_lossy(&cargo_status.stderr));
    }

    stdout_matches && stderr_matches
}

fn compare_gzip_decompress_outputs(args: &[&str], input_file: &str) -> bool {
    // Ensure target directory exists
    fs::create_dir_all("target").expect("Failed to create target directory");

    // Create temporary directory
    let temp_dir = tempfile::tempdir().expect("Failed to create temp directory");
    
    // Define paths
    let compressed_file = temp_dir.path().join("compressed.gz");
    let system_decompressed = temp_dir.path().join("system_decompressed");
    let cargo_decompressed = temp_dir.path().join("cargo_decompressed");

    // Compress input file using system gzip
    Command::new("gzip")
        .args(&["-c", "-r"])
        .arg(input_file)
        .stdout(File::create(&compressed_file).unwrap())
        .status()
        .expect("Failed to compress with system gzip");

    // Build the project
    Command::new("cargo")
        .arg("build")
        .status()
        .expect("Failed to build project");

    // System gzip decompression
    let gzip_status = Command::new("gzip")
        .args(args)
        .arg("-c")
        .arg(&compressed_file)
        .stdout(File::create(&system_decompressed).unwrap())
        .output()
        .expect("Failed to execute system gzip");

    // Our implementation decompression
    let cargo_status = Command::new("./target/debug/gzip")
        .args(args)
        .arg("-c")
        .arg(&compressed_file)
        .stdout(File::create(&cargo_decompressed).unwrap())
        .output()
        .expect("Failed to execute our gzip");

    // Compare outputs
    let stdout_matches = gzip_status.stdout == cargo_status.stdout;
    let stderr_matches = gzip_status.stderr == cargo_status.stderr;
    let file_matches = fs::read(&system_decompressed).unwrap() == fs::read(&cargo_decompressed).unwrap();

    if !stdout_matches || !stderr_matches || !file_matches {
        println!("\nDecompression comparison failed!");
        println!("System gzip stderr: {:?}", String::from_utf8_lossy(&gzip_status.stderr));
        println!("Our gzip stderr: {:?}", String::from_utf8_lossy(&cargo_status.stderr));
        
        // Save outputs for debugging
        fs::write("target/system_decompressed_output", fs::read(&system_decompressed).unwrap()).ok();
        fs::write("target/cargo_decompressed_output", fs::read(&cargo_decompressed).unwrap()).ok();
    }

    stdout_matches && stderr_matches 
}

#[test]
fn test_no_args() {
    assert!(compare_gzip_outputs(&[" "], None));
}

#[test]
fn test_nonexistent_file() {
    assert!(compare_gzip_outputs(&["-k", "-1", "test.txt"], None));
}

#[test]
fn test_forced_overwrite() {
    // Create test file
    let test_path = "tests/test-word.txt";
    let gz_path = format!("{}.gz", test_path);
    
    // Ensure .gz file exists
    File::create(&gz_path).unwrap();
    
    assert!(compare_gzip_outputs(&["-k", "-f", "-1", test_path], Some(test_path)));
    
    // Cleanup
    fs::remove_file(&gz_path).ok();
}

#[test]
fn test_file_deletion() {
    let test_path = "tests/test-temp.txt";
    let gz_path = format!("{}.gz", test_path);
    
    // Create test file with content
    let mut file = File::create(test_path).unwrap();
    file.write_all(b"test").unwrap();
    
    // Run our gzip implementation
    Command::new("./target/debug/gzip")
        .args(&["-f", "-1", test_path])
        .output()
        .expect("Failed to execute gzip");
    
    assert!(!Path::new(test_path).exists(), "File should be deleted");
    
    // Cleanup: remove the generated .gz file
    fs::remove_file(&gz_path).ok();
}

#[test]
fn test_help_menu() {
    assert!(compare_gzip_outputs(&["-h"], None));
}

#[test]
fn test_compression_level_1() {
    assert!(compare_gzip_outputs(
        &["-k","-f", "-1", "tests/test-word.txt"],
        Some("tests/test-word.txt")
    ));
}

#[test]
fn test_empty_bits_operand() {
    assert!(compare_gzip_outputs(&["-b"], None));
}

#[test]
fn test_incorrect_bits_operand() {
    assert!(compare_gzip_outputs(&["-b", "test"], None));
}

#[test]
fn test_bits_operand() {
    assert!(compare_gzip_outputs(
        &["-k", "-f", "-1", "-b", "3", "tests/test-word.txt"],
        Some("tests/test-word.txt")
    ));
}

#[test]
fn test_compression_level_2() {
    assert!(compare_gzip_outputs(
        &["-k", "-f", "-2", "tests/test-word.txt"],
        Some("tests/test-word.txt")
    ));
}

#[test]
fn test_compression_level_3() {
    assert!(compare_gzip_outputs(
        &["-k", "-f", "-3", "tests/test-word.txt"],
        Some("tests/test-word.txt")
    ));
}

#[test]
fn test_ascii_mode() {
    assert!(compare_gzip_outputs(
        &["-k", "-f", "-a", "-1", "tests/test-word.txt"],
        Some("tests/test-word.txt")
    ));
}

#[test]
fn test_stdout_mode() {
    // Use unique file names for this test
    let stdout_output = "tests/stdout_output.gz";
    let stdout_reference = "tests/stdout_reference.gz";
    let stdout_decoded = "tests/stdout_decoded.txt";
    let stdout_reference_decoded = "tests/stdout_reference_decoded.txt";
    
    // Clean up any existing files first
    for file in [stdout_output, stdout_reference] {
        fs::remove_file(file).ok();
    }

    // Run system gzip
    Command::new("gzip")
        .args(&["-c", "-1", "tests/test-word.txt"])
        .stdout(File::create(stdout_reference).unwrap())
        .spawn()
        .expect("Failed to execute system gzip")
        .wait()
        .expect("Failed to wait for system gzip");

    // Run our implementation
    Command::new("./target/debug/gzip")
        .args(&["-c", "-1", "tests/test-word.txt"])
        .stdout(File::create(stdout_output).unwrap())
        .spawn()
        .expect("Failed to execute our gzip")
        .wait()
        .expect("Failed to wait for our gzip");

    // Decompress both files using system gzip
    Command::new("gzip")
        .args(&["-d", "-c"])
        .stdin(File::open(stdout_reference).unwrap())
        .stdout(File::create(stdout_reference_decoded).unwrap())
        .spawn()
        .expect("Failed to decompress reference")
        .wait()
        .expect("Failed to wait for reference decompression");

    Command::new("gzip")
        .args(&["-d", "-c"])
        .stdin(File::open(stdout_output).unwrap())
        .stdout(File::create(stdout_decoded).unwrap())
        .spawn()
        .expect("Failed to decompress test output")
        .wait()
        .expect("Failed to wait for test decompression");

    // Compare decompressed contents
    let reference_content = fs::read_to_string(stdout_reference_decoded)
        .expect("Failed to read reference output");
    let test_content = fs::read_to_string(stdout_decoded)
        .expect("Failed to read test output");

    // Clean up
    for file in [stdout_output, stdout_reference, stdout_decoded, stdout_reference_decoded] {
        fs::remove_file(file).ok();
    }

    assert_eq!(reference_content, test_content, "Decompressed contents don't match");
}

#[test]
fn test_quiet_mode() {
    assert!(compare_gzip_outputs(
        &["-k", "-f", "-q", "-1", "tests/test-word.txt"],
        Some("tests/test-word.txt")
    ));
}

#[test]
fn test_no_name_mode() {
    assert!(compare_gzip_outputs(
        &["-k", "-f", "-n", "-1", "tests/test-word.txt"],
        Some("tests/test-word.txt")
    ));
}

#[test]
fn test_large_arg_combinations() {
    assert!(compare_gzip_outputs(
        &["-k", "-f", "-a", "-b", "3", "-q", "-n", "-1", "tests/test-word.txt"],
        Some("tests/test-word.txt")
    ));
}

#[test]
fn test_recursive() {
    assert!(compare_gzip_outputs(
        &["-r", "-f", "-1", "tests/testing"],
        Some("tests/testing")
    ));
}

// #[test]
fn test_combined_options() {
    assert!(compare_gzip_outputs(
        &["-1c", "tests/test-word.txt"],
        Some("tests/test-word.txt")
    ));
}

// #[test]
fn test_suffix_option() {
    assert!(compare_gzip_outputs(
        &["-k", "-f", "-S", ".gzip", "tests/test-word.txt"],
        Some("tests/test-word.txt")
    ));
}

#[test]
fn test_stdin_compression() {
    // Use unique file names for this test
    let stdin_output = "tests/stdin_output.gz";
    let stdin_reference = "tests/stdin_reference.gz";
    let stdin_decoded = "tests/stdin_decoded.txt";
    let stdin_reference_decoded = "tests/stdin_reference_decoded.txt";

    // Clean up any existing files first
    for file in [stdin_output, stdin_reference, stdin_decoded, stdin_reference_decoded] {
        fs::remove_file(file).ok();
    }

    // Create reference gzip output using system gzip
    let input = File::open("tests/test-word.txt").unwrap();
    let output = File::create(stdin_reference).unwrap();
    let mut cmd = Command::new("gzip")
        .args(&["-1"])
        .stdin(input)
        .stdout(output)
        .spawn()
        .expect("Failed to create reference output");
    cmd.wait().expect("Failed to wait for gzip");

    // Create output using our implementation
    let input = File::open("tests/test-word.txt").unwrap();
    let output = File::create(stdin_output).unwrap();
    let mut cmd = Command::new("./target/debug/gzip")
        .args(&["-1", "-f"])
        .stdin(input)
        .stdout(output)
        .spawn()
        .expect("Failed to create test output");
    cmd.wait().expect("Failed to wait for our gzip");

    // Decompress both files
    let output = File::create(stdin_reference_decoded).unwrap();
    let mut cmd = Command::new("gzip")
        .args(&["-dc", stdin_reference])
        .stdout(output)
        .spawn()
        .expect("Failed to decompress reference output");
    cmd.wait().expect("Failed to wait for decompression");

    let output = File::create(stdin_decoded).unwrap();
    let mut cmd = Command::new("gzip")
        .args(&["-dc", stdin_output])
        .stdout(output)
        .spawn()
        .expect("Failed to decompress test output");
    cmd.wait().expect("Failed to wait for decompression");

    // Compare decompressed contents
    let reference_content = fs::read_to_string(stdin_reference_decoded)
        .expect("Failed to read reference output");
    let test_content = fs::read_to_string(stdin_decoded)
        .expect("Failed to read test output");

    // Clean up
    for file in [stdin_output, stdin_reference, stdin_decoded, stdin_reference_decoded] {
        fs::remove_file(file).ok();
    }

    assert_eq!(reference_content, test_content, "Decompressed contents don't match");
}

#[test]
fn test_version() {
    assert!(compare_gzip_outputs(&["-L"], None));
}

#[test]
fn test_reproducible_compression() {
    // Use unique file names for this test
    let repro_input = "tests/repro_input.txt";
    let repro_comp1 = "tests/repro_comp1.gz";
    let repro_comp2 = "tests/repro_comp2.gz";

    // Clean up any existing files
    for file in [repro_input, repro_comp1, repro_comp2] {
        fs::remove_file(file).ok();
    }

    // Create input file
    let mut file = File::create(repro_input).unwrap();
    write!(file, "test data").unwrap();
    
    // First compression
    Command::new("./target/debug/gzip")
        .args(&["-k", "-1", "-c", "-n"])
        .stdin(File::open(repro_input).unwrap())
        .stdout(File::create(repro_comp1).unwrap())
        .spawn()
        .expect("Failed to start first compression")
        .wait()
        .expect("Failed first compression");

    // Wait a second to ensure different timestamp
    std::thread::sleep(std::time::Duration::from_secs(1));

    // Second compression
    Command::new("./target/debug/gzip")
        .args(&["-k", "-1", "-c", "-n"])
        .stdin(File::open(repro_input).unwrap())
        .stdout(File::create(repro_comp2).unwrap())
        .spawn()
        .expect("Failed to start second compression")
        .wait()
        .expect("Failed second compression");

    // Read and compare the contents of both compressed files
    let content1 = fs::read(repro_comp1).expect("Failed to read first compression");
    let content2 = fs::read(repro_comp2).expect("Failed to read second compression");

    // Clean up
    for file in [repro_input, repro_comp1, repro_comp2] {
        fs::remove_file(file).ok();
    }

    // Skip timestamp comparison (bytes 4-7) and compare the rest
    assert_eq!(&content1[0..4], &content2[0..4], "Header magic number and method should match");
    assert_eq!(&content1[8..], &content2[8..], "Compressed data should match");
}

// Helper function to generate test string of specified size
fn generate_test_string(size: usize) -> String {
    let base = "0123456789abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ_-+=%";
    let mut result = base.to_string();
    for _ in 0..10 {
        result = result.repeat(2);
    }
    result.chars().take(size).collect()
}

#[test]
fn test_size_0() {
    test_compression_with_size(0);
}

#[test]
fn test_size_1() {
    test_compression_with_size(1);
}

#[test]
fn test_size_2() {
    test_compression_with_size(2);
}

#[test]
fn test_size_3() {
    test_compression_with_size(3);
}

#[test]
fn test_size_4() {
    test_compression_with_size(4);
}

#[test]
fn test_size_32831() {
    test_compression_with_size(32831);
}

#[test]
fn test_size_32832() {
    test_compression_with_size(32832);
}

#[test]
fn test_size_32833() {
    test_compression_with_size(32833);
}

// #[test]
fn test_size_131071() {
    test_compression_with_size(131071);
}

// #[test]
fn test_size_131072() {
    test_compression_with_size(131072);
}

// #[test]
fn test_size_131073() {
    test_compression_with_size(131073);
}

fn test_compression_with_size(size: usize) {
    // Create temporary files
    let mut input_file = NamedTempFile::new().unwrap();
    let compressed_file = NamedTempFile::new().unwrap();
    let decompressed_file = NamedTempFile::new().unwrap();

    // Generate and write test data
    let test_data = generate_test_string(size);
    write!(input_file, "{}", test_data).unwrap();

    // Compress with our implementation
    Command::new("./target/debug/gzip")
        .args(&["-c", "-1"])
        .stdin(File::open(input_file.path()).unwrap())
        .stdout(Stdio::from(compressed_file.reopen().unwrap()))
        .spawn()
        .expect("Failed to start compression")
        .wait()
        .expect("Failed compression");

    // Decompress with system gzip
    Command::new("gzip")
        .args(&["-d", "-c"])
        .stdin(File::open(compressed_file.path()).unwrap())
        .stdout(Stdio::from(decompressed_file.reopen().unwrap()))
        .spawn()
        .expect("Failed to start decompression")
        .wait()
        .expect("Failed decompression");

    // Compare original and decompressed content
    let original_content = fs::read_to_string(input_file.path()).unwrap();
    let decompressed_content = fs::read_to_string(decompressed_file.path()).unwrap();
    
    assert_eq!(
        original_content.len(), 
        decompressed_content.len(), 
        "Content length mismatch for size {}. Expected length {}, got {}", 
        size,
        original_content.len(),
        decompressed_content.len()
    );

    assert!(
        original_content == decompressed_content,
        "Content mismatch for size {}. First 50 chars:\nOriginal: {:?}\nDecompressed: {:?}", 
        size,
        original_content.chars().take(50).collect::<String>(),
        decompressed_content.chars().take(50).collect::<String>()
    );
}

#[test]
fn test_decompress_help() {
    assert!(compare_gzip_outputs(&["-d", "-h"], None));
}

#[test]
fn test_decompress_no_args() {
    assert!(compare_gzip_outputs(&["-d"], None));
}

#[test]
fn test_decompress_nonexistent() {
    assert!(compare_gzip_outputs(&["-d", "test.txt.gz"], None));
}

#[test]
fn test_quiet_mode_decompress() {
    let test_file = "tests/test-word.txt";
    assert!(compare_gzip_decompress_outputs(&["-d", "-q"], test_file));
}

#[test]
fn test_force_decompress() {
    let test_file = "tests/test-word.txt";
    assert!(compare_gzip_decompress_outputs(&["-d", "-f"], test_file));
}

#[test]
fn test_stdout_mode_decompress() {
    let test_file = "tests/test-word.txt";
    assert!(compare_gzip_decompress_outputs(&["-d", "-c"], test_file));
}

#[test]
fn test_recursive_decompress() {
    // Create test directory structure
    fs::create_dir_all("tests/testing_decomp").expect("Failed to create test directory");
    fs::copy("tests/test-word.txt", "tests/testing_decomp/test-word.txt")
        .expect("Failed to copy test file");

    // Compress the file first using system gzip
    Command::new("gzip")
        .args(&["-k", "-1"])
        .arg("tests/testing_decomp/test-word.txt")
        .status()
        .expect("Failed to create test compressed file");

    // Run the decompression test
    assert!(compare_gzip_decompress_outputs(
        &["-d", "-r"],
        "tests/testing_decomp"
    ));

    // Cleanup
    fs::remove_dir_all("tests/testing_decomp").expect("Failed to cleanup test directory");
}

#[test]
fn test_no_name_mode_decompress() {
    let test_file = "tests/test-word.txt";
    assert!(compare_gzip_decompress_outputs(&["-d", "-n"], test_file));
}

#[test]
fn test_combined_parameters_decompress() {
    let test_file = "tests/test-word.txt";
    assert!(compare_gzip_decompress_outputs(
        &["-d", "-f", "-n", "-q"],
        test_file
    ));
}

#[test]
fn test_non_integer_parameter_decompress() {
    let test_file = "tests/test-word.txt";
    assert!(compare_gzip_decompress_outputs(&["-d", "-b"], test_file));
}

#[test]
fn test_level_1_decompress() {
    let test_file = "tests/test-word.txt";
    assert!(compare_gzip_decompress_outputs(&["-d", "-1"], test_file));
}

#[test]
fn test_level_2_decompress() {
    let test_file = "tests/test-word.txt";
    assert!(compare_gzip_decompress_outputs(&["-d", "-2"], test_file));
}

#[test]
fn test_level_3_decompress() {
    let test_file = "tests/test-word.txt";
    assert!(compare_gzip_decompress_outputs(&["-d", "-3"], test_file));
}

#[test]
fn test_combined_options_cdf_decompress() {
    let test_file = "tests/test-word.txt";
    assert!(compare_gzip_decompress_outputs(&["-c", "-d", "-f"], test_file));
}

#[test]
fn test_suffix_option_decompress() {
    let test_file = "tests/test-word.txt";
    assert!(compare_gzip_decompress_outputs(&["-d", "-S", ".gzip"], test_file));
}

#[test]
fn test_gzip_env_valid_option() {
    // Create test file
    let mut file = File::create("test-word.txt").expect("Failed to create test file");
    write!(file, "test data").expect("Failed to write test data");

    // Compress the file
    Command::new("gzip")
        .args(&["-k"])
        .arg("test-word.txt")
        .status()
        .expect("Failed to compress test file");

    // Create temp files for output comparison
    let gzip_output = NamedTempFile::new().unwrap();
    let cargo_output = NamedTempFile::new().unwrap();

    // Run system gzip with GZIP env var
    let gzip_status = Command::new("gzip")
        .arg("-d")
        .env("GZIP", "-1")
        .stdin(File::open("test-word.txt.gz").unwrap())
        .stdout(Stdio::from(gzip_output.reopen().unwrap()))
        .output()
        .expect("Failed to execute system gzip");

    // Run our implementation with GZIP env var
    let cargo_status = Command::new("./target/debug/gzip")
        .arg("-d")
        .env("GZIP", "-1")
        .stdin(File::open("test-word.txt.gz").unwrap())
        .stdout(Stdio::from(cargo_output.reopen().unwrap()))
        .output()
        .expect("Failed to execute our gzip");

    // Compare outputs
    let stdout_matches = gzip_status.stdout == cargo_status.stdout;
    let stderr_matches = gzip_status.stderr == cargo_status.stderr;
    
    if !stdout_matches || !stderr_matches {
        fs::write("target/gzip_env_good_output.txt", &gzip_status.stderr).ok();
        fs::write("target/cargo_env_good_output.txt", &cargo_status.stderr).ok();
    }

    // Cleanup
    fs::remove_file("test-word.txt").ok();
    fs::remove_file("test-word.txt.gz").ok();

    assert!(stdout_matches && stderr_matches);
}

#[test]
fn test_gzip_env_invalid_option() {
    // Create test file
    let mut file = File::create("test-word.txt").expect("Failed to create test file");
    write!(file, "test data").expect("Failed to write test data");

    // Compress the file
    Command::new("gzip")
        .args(&["-k"])
        .arg("test-word.txt")
        .status()
        .expect("Failed to compress test file");

    // Create temp files for output comparison
    let gzip_output = NamedTempFile::new().unwrap();
    let cargo_output = NamedTempFile::new().unwrap();

    // Run system gzip with invalid GZIP env var
    let gzip_status = Command::new("gzip")
        .arg("-d")
        .env("GZIP", "--stdout")
        .stdin(File::open("test-word.txt.gz").unwrap())
        .stdout(Stdio::from(gzip_output.reopen().unwrap()))
        .output()
        .expect("Failed to execute system gzip");

    // Run our implementation with invalid GZIP env var
    let cargo_status = Command::new("./target/debug/gzip")
        .arg("-d")
        .env("GZIP", "--stdout")
        .stdin(File::open("test-word.txt.gz").unwrap())
        .stdout(Stdio::from(cargo_output.reopen().unwrap()))
        .output()
        .expect("Failed to execute our gzip");

    // Compare outputs
    let stdout_matches = gzip_status.stdout == cargo_status.stdout;
    let stderr_matches = gzip_status.stderr == cargo_status.stderr;
    
    if !stdout_matches || !stderr_matches {
        fs::write("target/gzip_env_bad_output.txt", &gzip_status.stderr).ok();
        fs::write("target/cargo_env_bad_output.txt", &cargo_status.stderr).ok();
    }

    // Cleanup
    fs::remove_file("test-word.txt").ok();
    fs::remove_file("test-word.txt.gz").ok();

    assert!(stdout_matches && stderr_matches);
}

#[test]
fn test_decompress_size_0() {
    test_decompression_with_size(0);
}

#[test]
fn test_decompress_size_1() {
    test_decompression_with_size(1);
}

#[test]
fn test_decompress_size_2() {
    test_decompression_with_size(2);
}

#[test]
fn test_decompress_size_3() {
    test_decompression_with_size(3);
}

#[test]
fn test_decompress_size_4() {
    test_decompression_with_size(4);
}

#[test]
fn test_decompress_size_32831() {
    test_decompression_with_size(32831);
}

#[test]
fn test_decompress_size_32832() {
    test_decompression_with_size(32832);
}

#[test]
fn test_decompress_size_32833() {
    test_decompression_with_size(32833);
}

#[test]
fn test_decompress_size_131071() {
    test_decompression_with_size(131071);
}

#[test]
fn test_decompress_size_131072() {
    test_decompression_with_size(131072);
}

#[test]
fn test_decompress_size_131073() {
    test_decompression_with_size(131073);
}

fn test_decompression_with_size(size: usize) {
    // Create temporary files
    let mut input_file = NamedTempFile::new().unwrap();
    let compressed_file = NamedTempFile::new().unwrap();
    let decompressed_file = NamedTempFile::new().unwrap();

    // Generate and write test data
    let test_data = generate_test_string(size);
    write!(input_file, "{}", test_data).unwrap();

    // Compress with system gzip
    Command::new("gzip")
        .args(&["-c", "-1"])
        .stdin(File::open(input_file.path()).unwrap())
        .stdout(Stdio::from(compressed_file.reopen().unwrap()))
        .spawn()
        .expect("Failed to start compression")
        .wait()
        .expect("Failed compression");

    // Decompress with our implementation
    Command::new("./target/debug/gzip")
        .args(&["-d", "-c"])
        .stdin(File::open(compressed_file.path()).unwrap())
        .stdout(Stdio::from(decompressed_file.reopen().unwrap()))
        .spawn()
        .expect("Failed to start decompression")
        .wait()
        .expect("Failed decompression");

    // Compare original and decompressed content
    let original_content = fs::read_to_string(input_file.path()).unwrap();
    let decompressed_content = fs::read_to_string(decompressed_file.path()).unwrap();
    
    assert_eq!(
        original_content.len(), 
        decompressed_content.len(), 
        "Content length mismatch for size {}. Expected length {}, got {}", 
        size,
        original_content.len(),
        decompressed_content.len()
    );

    assert!(
        original_content == decompressed_content,
        "Content mismatch for size {}. First 50 chars:\nOriginal: {:?}\nDecompressed: {:?}", 
        size,
        original_content.chars().take(50).collect::<String>(),
        decompressed_content.chars().take(50).collect::<String>()
    );
}

#[test]
fn test_huft_segv_decompression() {
    // Create temporary files for outputs
    let gzip_output = NamedTempFile::new().unwrap();
    let cargo_output = NamedTempFile::new().unwrap();

    // Run system gzip
    let gzip_status = Command::new("gzip")
        .args(&["-d"])
        .stdin(File::open("hufts-segv.gz").unwrap())
        .stdout(Stdio::from(gzip_output.reopen().unwrap()))
        .stderr(Stdio::inherit())
        .status()
        .expect("Failed to execute system gzip");

    // Run our implementation
    let cargo_status = Command::new("./target/debug/gzip")
        .args(&["-d"])
        .stdin(File::open("hufts-segv.gz").unwrap())
        .stdout(Stdio::from(cargo_output.reopen().unwrap()))
        .stderr(Stdio::inherit())
        .status()
        .expect("Failed to execute our gzip");

    // Compare exit status codes
    assert_eq!(
        gzip_status.code(),
        cargo_status.code(),
        "Exit status codes don't match"
    );

    // Compare outputs
    let gzip_content = fs::read(gzip_output.path()).unwrap_or_default();
    let cargo_content = fs::read(cargo_output.path()).unwrap_or_default();

    if gzip_content != cargo_content {
        // Save outputs for debugging if they don't match
        fs::write("target/gzip_huft_segv_output.txt", &gzip_content).ok();
        fs::write("target/cargo_huft_segv_output.txt", &cargo_content).ok();
        panic!("Output content doesn't match");
    }
}

// #[test]
fn test_bug33501_decompression() {
    // Create temporary files
    let test_file = NamedTempFile::new().unwrap();
    let gzip_output = NamedTempFile::new().unwrap();
    let cargo_output = NamedTempFile::new().unwrap();

    // Create the invalid gzip file
    let invalid_gzip_data = [
        0x1f, 0x8b,  // Magic number
        0x08,        // Compression method (deflate)
        0x00,        // Flags
        0x30, 0x30, 0x30, 0x30, 0x30, 0x30,  // Modification time
        0x64,        // Extra flags
        0x00,        // Operating system
        0x00, 0x00   // Extra data
    ];
    fs::write(test_file.path(), &invalid_gzip_data).unwrap();

    // Run system gzip
    let gzip_status = Command::new("gzip")
        .args(&["-d"])
        .stdin(File::open(test_file.path()).unwrap())
        .stdout(Stdio::from(gzip_output.reopen().unwrap()))
        .stderr(Stdio::inherit())
        .status()
        .expect("Failed to execute system gzip");

    // Run our implementation
    let cargo_status = Command::new("./target/debug/gzip")
        .args(&["-d"])
        .stdin(File::open(test_file.path()).unwrap())
        .stdout(Stdio::from(cargo_output.reopen().unwrap()))
        .stderr(Stdio::inherit())
        .status()
        .expect("Failed to execute our gzip");

    // Compare exit status codes
    assert_eq!(
        gzip_status.code(),
        cargo_status.code(),
        "Exit status codes don't match"
    );

    // Compare outputs
    let gzip_content = fs::read(gzip_output.path()).unwrap_or_default();
    let cargo_content = fs::read(cargo_output.path()).unwrap_or_default();

    if gzip_content != cargo_content {
        // Save outputs for debugging if they don't match
        fs::write("target/gzip_bug33501_output.txt", &gzip_content).ok();
        fs::write("target/cargo_bug33501_output.txt", &cargo_content).ok();
        panic!("Output content doesn't match");
    }
}

// #[test]
fn test_pure_uncompressed_data() {
    // Create temporary files
    let mut input_file = NamedTempFile::new().unwrap();
    let gzip_output = NamedTempFile::new().unwrap();
    let cargo_output = NamedTempFile::new().unwrap();

    // Create test input data
    writeln!(input_file, "xxx").unwrap();
    writeln!(input_file, "yyy").unwrap();

    // Run system gzip
    let gzip_status = Command::new("gzip")
        .args(&["-c", "-d", "-f"])
        .stdin(File::open(input_file.path()).unwrap())
        .stdout(Stdio::from(gzip_output.reopen().unwrap()))
        .stderr(Stdio::inherit())
        .status()
        .expect("Failed to execute system gzip");

    // Run our implementation
    let cargo_status = Command::new("./target/debug/gzip")
        .args(&["-c", "-d", "-f"])
        .stdin(File::open(input_file.path()).unwrap())
        .stdout(Stdio::from(cargo_output.reopen().unwrap()))
        .stderr(Stdio::inherit())
        .status()
        .expect("Failed to execute our gzip");

    // Compare exit status codes
    assert_eq!(
        gzip_status.code(),
        cargo_status.code(),
        "Exit status codes don't match"
    );

    // Compare outputs
    let gzip_content = fs::read_to_string(gzip_output.path()).unwrap_or_default();
    let cargo_content = fs::read_to_string(cargo_output.path()).unwrap_or_default();

    if gzip_content != cargo_content {
        // Save outputs for debugging if they don't match
        fs::write("target/gzip_pure_uncomp_output.txt", &gzip_content).ok();
        fs::write("target/cargo_pure_uncomp_output.txt", &cargo_content).ok();
        panic!("Output content doesn't match.\nExpected:\n{}\nGot:\n{}", gzip_content, cargo_content);
    }
}

// #[test]
fn test_compressed_followed_by_uncompressed() {
    // Create temporary files
    let input_file = NamedTempFile::new().unwrap();
    let gzip_output = NamedTempFile::new().unwrap();
    let cargo_output = NamedTempFile::new().unwrap();
    let temp_compressed = NamedTempFile::new().unwrap();

    // Create compressed part
    Command::new("gzip")
        .stdin(Stdio::piped())
        .stdout(Stdio::from(temp_compressed.reopen().unwrap()))
        .spawn()
        .unwrap()
        .stdin
        .unwrap()
        .write_all(b"xxx\n")
        .unwrap();

    // Create the combined input file (compressed + uncompressed)
    {
        let mut combined = File::create(input_file.path()).unwrap();
        // Write compressed data
        let compressed_data = fs::read(temp_compressed.path()).unwrap();
        combined.write_all(&compressed_data).unwrap();
        // Write uncompressed data
        writeln!(combined, "yyy").unwrap();
    }

    // Run system gzip
    let gzip_status = Command::new("gzip")
        .args(&["-c", "-d", "-f"])
        .stdin(File::open(input_file.path()).unwrap())
        .stdout(Stdio::from(gzip_output.reopen().unwrap()))
        .stderr(Stdio::inherit())
        .status()
        .expect("Failed to execute system gzip");

    // Run our implementation
    let cargo_status = Command::new("./target/debug/gzip")
        .args(&["-c", "-d", "-f"])
        .stdin(File::open(input_file.path()).unwrap())
        .stdout(Stdio::from(cargo_output.reopen().unwrap()))
        .stderr(Stdio::inherit())
        .status()
        .expect("Failed to execute our gzip");

    // Compare exit status codes
    assert_eq!(
        gzip_status.code(),
        cargo_status.code(),
        "Exit status codes don't match"
    );

    // Compare outputs
    let gzip_content = fs::read_to_string(gzip_output.path()).unwrap_or_default();
    let cargo_content = fs::read_to_string(cargo_output.path()).unwrap_or_default();

    if gzip_content != cargo_content {
        // Save outputs for debugging if they don't match
        fs::write("target/gzip_comp_uncomp_output.txt", &gzip_content).ok();
        fs::write("target/cargo_comp_uncomp_output.txt", &cargo_content).ok();
        panic!("Output content doesn't match.\nExpected:\n{}\nGot:\n{}", gzip_content, cargo_content);
    }
}

// #[test]
fn test_double_compressed_data() {
    // Create temporary files
    let input_file = NamedTempFile::new().unwrap();
    let gzip_output = NamedTempFile::new().unwrap();
    let cargo_output = NamedTempFile::new().unwrap();
    let temp_compressed1 = NamedTempFile::new().unwrap();
    let temp_compressed2 = NamedTempFile::new().unwrap();

    // Create first compressed part
    Command::new("gzip")
        .stdin(Stdio::piped())
        .stdout(Stdio::from(temp_compressed1.reopen().unwrap()))
        .spawn()
        .unwrap()
        .stdin
        .unwrap()
        .write_all(b"xxx\n")
        .unwrap();

    // Create second compressed part
    Command::new("gzip")
        .stdin(Stdio::piped())
        .stdout(Stdio::from(temp_compressed2.reopen().unwrap()))
        .spawn()
        .unwrap()
        .stdin
        .unwrap()
        .write_all(b"yyy\n")
        .unwrap();

    // Create the combined input file (both compressed parts)
    {
        let mut combined = File::create(input_file.path()).unwrap();
        // Write first compressed data
        let compressed_data1 = fs::read(temp_compressed1.path()).unwrap();
        combined.write_all(&compressed_data1).unwrap();
        // Write second compressed data
        let compressed_data2 = fs::read(temp_compressed2.path()).unwrap();
        combined.write_all(&compressed_data2).unwrap();
    }

    // Run system gzip
    let gzip_status = Command::new("gzip")
        .args(&["-c", "-d", "-f"])
        .stdin(File::open(input_file.path()).unwrap())
        .stdout(Stdio::from(gzip_output.reopen().unwrap()))
        .stderr(Stdio::inherit())
        .status()
        .expect("Failed to execute system gzip");

    // Run our implementation
    let cargo_status = Command::new("./target/debug/gzip")
        .args(&["-c", "-d", "-f"])
        .stdin(File::open(input_file.path()).unwrap())
        .stdout(Stdio::from(cargo_output.reopen().unwrap()))
        .stderr(Stdio::inherit())
        .status()
        .expect("Failed to execute our gzip");

    // Compare exit status codes
    assert_eq!(
        gzip_status.code(),
        cargo_status.code(),
        "Exit status codes don't match"
    );

    // Compare outputs
    let gzip_content = fs::read_to_string(gzip_output.path()).unwrap_or_default();
    let cargo_content = fs::read_to_string(cargo_output.path()).unwrap_or_default();

    if gzip_content != cargo_content {
        // Save outputs for debugging if they don't match
        fs::write("target/gzip_double_comp_output.txt", &gzip_content).ok();
        fs::write("target/cargo_double_comp_output.txt", &cargo_content).ok();
        panic!("Output content doesn't match.\nExpected:\n{}\nGot:\n{}", gzip_content, cargo_content);
    }
}

#[test]
fn test_reproducible_decompression() {
    // Create temporary files
    let mut input_file = NamedTempFile::new().unwrap();
    let compressed_file = NamedTempFile::new().unwrap();
    let decomp1_file = NamedTempFile::new().unwrap();
    let decomp2_file = NamedTempFile::new().unwrap();

    // Create test input data
    write!(input_file, "test data\n").unwrap();

    // Compress the input file using system gzip
    Command::new("gzip")
        .args(&["-c"])
        .stdin(File::open(input_file.path()).unwrap())
        .stdout(Stdio::from(compressed_file.reopen().unwrap()))
        .status()
        .expect("Failed to compress test data");

    // First decompression
    Command::new("./target/debug/gzip")
        .args(&["-d", "-c"])
        .stdin(File::open(compressed_file.path()).unwrap())
        .stdout(Stdio::from(decomp1_file.reopen().unwrap()))
        .status()
        .expect("Failed first decompression");

    // Wait a second to ensure different timestamp
    std::thread::sleep(std::time::Duration::from_secs(1));

    // Second decompression
    Command::new("./target/debug/gzip")
        .args(&["-d", "-c"])
        .stdin(File::open(compressed_file.path()).unwrap())
        .stdout(Stdio::from(decomp2_file.reopen().unwrap()))
        .status()
        .expect("Failed second decompression");

    // Read and compare the contents of both decompressed files
    let content1 = fs::read_to_string(decomp1_file.path()).unwrap();
    let content2 = fs::read_to_string(decomp2_file.path()).unwrap();

    if content1 != content2 {
        // Save files for debugging if they don't match
        fs::write("target/decomp1", &content1).ok();
        fs::write("target/decomp2", &content2).ok();
        fs::copy(compressed_file.path(), "target/test.gz").ok();
        panic!(
            "Decompression results don't match.\nFirst decompression:\n{}\nSecond decompression:\n{}",
            content1,
            content2
        );
    }
}