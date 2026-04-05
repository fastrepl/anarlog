#[cfg(target_os = "macos")]
use objc2::{rc::Retained, runtime::NSObject};
#[cfg(target_os = "macos")]
use objc2_app_kit::{NSSharingService, NSSharingServiceNameComposeEmail};
#[cfg(target_os = "macos")]
use objc2_foundation::{NSArray, NSString, NSURL};
use std::path::Path;

pub struct EmailComposer;

#[cfg(target_os = "macos")]
/*
NSString* htmlText = @"<html><body>Hello, <b>World</b>!</body></html>";
NSData* textData = [NSData dataWithBytes:[htmlText UTF8String] length:[htmlText lengthOfBytesUsingEncoding:NSUTF8StringEncoding]];
NSAttributedString* textAttributedString = [[NSAttributedString alloc] initWithHTML:textData options:options documentAttributes:nil];

// create a file to attach
NSUUID* uuid = [NSUUID new];
NSString* tempDir = [NSTemporaryDirectory() stringByAppendingPathComponent:[uuid UUIDString]];
NSFileManager* fm = [NSFileManager new];
[fm createDirectoryAtPath:tempDir withIntermediateDirectories:YES attributes:nil error:nil];
NSString* tempFile = [tempDir stringByAppendingPathComponent:@"report.csv"];
NSURL* tempFileURL = [NSURL fileURLWithPath:tempFile];
NSData* csv = ...; // generate the data here
[csv writeToURL:tempFileURL atomically:NO];

// share it
NSSharingService* mailShare = [NSSharingService sharingServiceNamed:NSSharingServiceNameComposeEmail];
NSArray* shareItems = @[textAttributedString,tempFileURL];
[mailShare performWithItems:shareItems];
*/
impl EmailComposer {
    /// Open email client with HTML content and optional attachments
    pub fn open_email_client(
        recipients: &[&str],
        subject: &str,
        html_message: &str,
        attachments: &[&Path],
    ) -> Result<(), Box<dyn std::error::Error>> {
        unsafe {
            // Create sharing service for email composition
            let service = NSSharingService::sharingServiceNamed(NSSharingServiceNameComposeEmail)
                .ok_or("Email service not available")?;

            // Set recipients
            let ns_recipients = Self::create_ns_string_array(recipients)?;
            service.setRecipients(Some(&ns_recipients));

            // Set subject
            let ns_subject = NSString::from_str(subject);
            service.setSubject(Some(&ns_subject));

            // Create items array for sharing
            let items = Self::create_sharing_items(html_message, attachments)?;

            // Convert items to references for NSArray
            let item_refs: Vec<&NSObject> = items.iter().map(|item| &**item).collect();
            let items_array = NSArray::from_slice(&item_refs);

            // Convert to NSArray<AnyObject>
            let any_array =
                &*(&items_array as *const _ as *const NSArray<objc2::runtime::AnyObject>);

            // Check if service can perform with these items
            if !service.canPerformWithItems(Some(any_array)) {
                return Err("Cannot perform email composition with provided items".into());
            }

            // Open email client
            service.performWithItems(any_array);

            Ok(())
        }
    }

    /// Create NSArray of NSString from slice of string references
    unsafe fn create_ns_string_array(
        strings: &[&str],
    ) -> Result<Retained<NSArray<NSString>>, Box<dyn std::error::Error>> {
        let ns_strings: Vec<Retained<NSString>> =
            strings.iter().map(|s| NSString::from_str(s)).collect();
        let refs: Vec<&NSString> = ns_strings.iter().map(|s| &**s).collect();
        Ok(NSArray::from_slice(&refs))
    }

    /// Create items array with HTML content and attachments
    unsafe fn create_sharing_items(
        html_message: &str,
        attachments: &[&Path],
    ) -> Result<Vec<Retained<NSObject>>, Box<dyn std::error::Error>> {
        let mut items: Vec<Retained<NSObject>> = Vec::new();

        // Add HTML content as plain string for now
        let html_string = NSString::from_str(html_message);
        items.push(unsafe { Retained::cast_unchecked(html_string) });

        // Add attachments
        for &path in attachments {
            let file_url = unsafe { Self::create_file_url(path)? };
            items.push(unsafe { Retained::cast_unchecked(file_url) });
        }

        Ok(items)
    }

    /// Create NSURL from file path
    unsafe fn create_file_url(path: &Path) -> Result<Retained<NSURL>, Box<dyn std::error::Error>> {
        let path_str = path.to_str().ok_or("Invalid path")?;
        let ns_path = NSString::from_str(path_str);
        let file_url = NSURL::fileURLWithPath(&ns_path);

        if !file_url.isFileURL() {
            return Err("Not a valid file URL".into());
        }

        Ok(file_url)
    }

    /// Create a temporary file with given content (helper for attachments)
    pub fn create_temp_file_with_content(
        filename: &str,
        content: &[u8],
    ) -> Result<std::path::PathBuf, Box<dyn std::error::Error>> {
        use std::fs::File;
        use std::io::Write;

        let temp_dir = std::env::temp_dir();
        let file_path = temp_dir.join(filename);

        let mut file = File::create(&file_path)?;
        file.write_all(content)?;

        Ok(file_path)
    }
}

#[cfg(not(target_os = "macos"))]
impl EmailComposer {
    /// Open email client with HTML content and optional attachments (stub for non-macOS)
    pub fn open_email_client(
        _recipients: &[&str],
        _subject: &str,
        _html_message: &str,
        _attachments: &[&Path],
    ) -> Result<(), Box<dyn std::error::Error>> {
        Err("Email composition is only supported on macOS".into())
    }

    /// Create a temporary file with given content (helper for attachments)
    pub fn create_temp_file_with_content(
        filename: &str,
        content: &[u8],
    ) -> Result<std::path::PathBuf, Box<dyn std::error::Error>> {
        use std::fs::File;
        use std::io::Write;

        let temp_dir = std::env::temp_dir();
        let file_path = temp_dir.join(filename);

        let mut file = File::create(&file_path)?;
        file.write_all(content)?;

        Ok(file_path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_email_client() -> Result<(), Box<dyn std::error::Error>> {
        let html_body = "<p>Simple test email</p>";

        EmailComposer::open_email_client(
            &["test@example.com"],
            "Simple Test",
            html_body,
            &[], // No attachments
        )?;

        println!("Email client opened successfully!");
        Ok(())
    }
}
