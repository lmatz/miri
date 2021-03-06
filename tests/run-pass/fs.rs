// ignore-windows: File handling is not implemented yet
// compile-flags: -Zmiri-disable-isolation

use std::fs::{File, remove_file, rename};
use std::io::{Read, Write, ErrorKind, Result, Seek, SeekFrom};
use std::path::{PathBuf, Path};

fn main() {
    test_file();
    test_file_clone();
    test_seek();
    test_metadata();
    test_symlink();
    test_errors();
    test_rename();
}

/// Prepare: compute filename and make sure the file does not exist.
fn prepare(filename: &str) -> PathBuf {
    let tmp = std::env::temp_dir();
    let path = tmp.join(filename);
    // Clean the paths for robustness.
    remove_file(&path).ok();
    path
}

/// Prepare like above, and also write some initial content to the file.
fn prepare_with_content(filename: &str, content: &[u8]) -> PathBuf {
    let path = prepare(filename);
    let mut file = File::create(&path).unwrap();
    file.write(content).unwrap();
    path
}

fn test_file() {
    let bytes = b"Hello, World!\n";
    let path = prepare("miri_test_fs_file.txt");

    // Test creating, writing and closing a file (closing is tested when `file` is dropped).
    let mut file = File::create(&path).unwrap();
    // Writing 0 bytes should not change the file contents.
    file.write(&mut []).unwrap();
    assert_eq!(file.metadata().unwrap().len(), 0);

    file.write(bytes).unwrap();
    assert_eq!(file.metadata().unwrap().len(), bytes.len() as u64);
    // Test opening, reading and closing a file.
    let mut file = File::open(&path).unwrap();
    let mut contents = Vec::new();
    // Reading 0 bytes should not move the file pointer.
    file.read(&mut []).unwrap();
    // Reading until EOF should get the whole text.
    file.read_to_end(&mut contents).unwrap();
    assert_eq!(bytes, contents.as_slice());

    // Removing file should succeed.
    remove_file(&path).unwrap();
}

fn test_file_clone() {
    let bytes = b"Hello, World!\n";
    let path = prepare_with_content("miri_test_fs_file_clone.txt", bytes);

    // Cloning a file should be successful.
    let file = File::open(&path).unwrap();
    let mut cloned = file.try_clone().unwrap();
    // Reading from a cloned file should get the same text.
    let mut contents = Vec::new();
    cloned.read_to_end(&mut contents).unwrap();
    assert_eq!(bytes, contents.as_slice());

    // Removing file should succeed.
    remove_file(&path).unwrap();
}

fn test_seek() {
    let bytes = b"Hello, entire World!\n";
    let path = prepare_with_content("miri_test_fs_seek.txt", bytes);

    let mut file = File::open(&path).unwrap();
    let mut contents = Vec::new();
    file.read_to_end(&mut contents).unwrap();
    assert_eq!(bytes, contents.as_slice());
    // Test that seeking to the beginning and reading until EOF gets the text again.
    file.seek(SeekFrom::Start(0)).unwrap();
    let mut contents = Vec::new();
    file.read_to_end(&mut contents).unwrap();
    assert_eq!(bytes, contents.as_slice());
    // Test seeking relative to the end of the file.
    file.seek(SeekFrom::End(-1)).unwrap();
    let mut contents = Vec::new();
    file.read_to_end(&mut contents).unwrap();
    assert_eq!(&bytes[bytes.len() - 1..], contents.as_slice());
    // Test seeking relative to the current position.
    file.seek(SeekFrom::Start(5)).unwrap();
    file.seek(SeekFrom::Current(-3)).unwrap();
    let mut contents = Vec::new();
    file.read_to_end(&mut contents).unwrap();
    assert_eq!(&bytes[2..], contents.as_slice());

    // Removing file should succeed.
    remove_file(&path).unwrap();
}

fn check_metadata(bytes: &[u8], path: &Path) -> Result<()> {
    // Test that the file metadata is correct.
    let metadata = path.metadata()?;
    // `path` should point to a file.
    assert!(metadata.is_file());
    // The size of the file must be equal to the number of written bytes.
    assert_eq!(bytes.len() as u64, metadata.len());
    Ok(())
}

fn test_metadata() {
    let bytes = b"Hello, meta-World!\n";
    let path = prepare_with_content("miri_test_fs_metadata.txt", bytes);

    // Test that metadata of an absolute path is correct.
    check_metadata(bytes, &path).unwrap();
    // Test that metadata of a relative path is correct.
    std::env::set_current_dir(path.parent().unwrap()).unwrap();
    check_metadata(bytes, Path::new(path.file_name().unwrap())).unwrap();

    // Removing file should succeed.
    remove_file(&path).unwrap();
}

fn test_symlink() {
    let bytes = b"Hello, World!\n";
    let path = prepare_with_content("miri_test_fs_link_target.txt", bytes);
    let symlink_path = prepare("miri_test_fs_symlink.txt");

    // Creating a symbolic link should succeed.
    std::os::unix::fs::symlink(&path, &symlink_path).unwrap();
    // Test that the symbolic link has the same contents as the file.
    let mut symlink_file = File::open(&symlink_path).unwrap();
    let mut contents = Vec::new();
    symlink_file.read_to_end(&mut contents).unwrap();
    assert_eq!(bytes, contents.as_slice());
    // Test that metadata of a symbolic link is correct.
    check_metadata(bytes, &symlink_path).unwrap();
    // Test that the metadata of a symbolic link is correct when not following it.
    assert!(symlink_path.symlink_metadata().unwrap().file_type().is_symlink());
    // Removing symbolic link should succeed.
    remove_file(&symlink_path).unwrap();

    // Removing file should succeed.
    remove_file(&path).unwrap();
}

fn test_errors() {
    let bytes = b"Hello, World!\n";
    let path = prepare("miri_test_fs_errors.txt");

    // The following tests also check that the `__errno_location()` shim is working properly.
    // Opening a non-existing file should fail with a "not found" error.
    assert_eq!(ErrorKind::NotFound, File::open(&path).unwrap_err().kind());
    // Removing a non-existing file should fail with a "not found" error.
    assert_eq!(ErrorKind::NotFound, remove_file(&path).unwrap_err().kind());
    // Reading the metadata of a non-existing file should fail with a "not found" error.
    assert_eq!(ErrorKind::NotFound, check_metadata(bytes, &path).unwrap_err().kind());
}

fn test_rename() {
    // Renaming a file should succeed.
    let path1 = prepare("miri_test_fs_rename_source.txt");
    let path2 = prepare("miri_test_fs_rename_destination.txt");

    let file = File::create(&path1).unwrap();
    drop(file);

    // Renaming should succeed
    rename(&path1, &path2).unwrap();
    // Check that the old file path isn't present
    assert_eq!(ErrorKind::NotFound, path1.metadata().unwrap_err().kind());
    // Check that the file has moved successfully
    assert!(path2.metadata().unwrap().is_file());

    // Renaming a nonexistent file should fail
    assert_eq!(ErrorKind::NotFound, rename(&path1, &path2).unwrap_err().kind());

    remove_file(&path2).unwrap();
}
