use std::path::Path;

/// A single indexed document with its extracted text and source path.
#[derive(Debug, Clone)]
pub struct IndexedDocument {
    /// Relative path from campaign root (e.g., "rules/docs/handbook.pdf")
    pub source: String,
    /// Extracted text content
    pub text: String,
}

/// In-memory search index built from PDF and markdown files.
#[derive(Debug, Default)]
pub struct SearchIndex {
    pub documents: Vec<IndexedDocument>,
}

impl SearchIndex {
    pub fn new() -> Self {
        Self {
            documents: Vec::new(),
        }
    }

    /// Index all PDF files from `rules/docs/` within the campaign directory.
    /// Missing directory is handled gracefully (returns empty index).
    pub fn index_pdfs(&mut self, campaign_path: &Path) {
        let docs_dir = campaign_path.join("rules").join("docs");
        let entries = match std::fs::read_dir(&docs_dir) {
            Ok(entries) => entries,
            Err(_) => return,
        };

        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            let ext = path.extension().and_then(|e| e.to_str());
            if ext != Some("pdf") {
                continue;
            }

            match extract_pdf_text(&path) {
                Ok(text) if !text.trim().is_empty() => {
                    let relative = path
                        .strip_prefix(campaign_path)
                        .unwrap_or(&path)
                        .to_string_lossy()
                        .to_string();
                    self.documents.push(IndexedDocument {
                        source: relative,
                        text,
                    });
                }
                _ => {}
            }
        }
    }
    /// Index all markdown files from the campaign: lore.md and dialogues.md
    /// from player and NPC folders. Missing directories are handled gracefully.
    pub fn index_markdown(&mut self, campaign_path: &Path) {
        for subdir in &["players", "npc"] {
            let dir = campaign_path.join(subdir);
            let entries = match std::fs::read_dir(&dir) {
                Ok(entries) => entries,
                Err(_) => continue,
            };

            for entry in entries.flatten() {
                let path = entry.path();
                if !path.is_dir() {
                    continue;
                }

                for md_file in &["lore.md", "dialogues.md"] {
                    let md_path = path.join(md_file);
                    if md_path.is_file() {
                        if let Ok(text) = std::fs::read_to_string(&md_path) {
                            if !text.trim().is_empty() {
                                let relative = md_path
                                    .strip_prefix(campaign_path)
                                    .unwrap_or(&md_path)
                                    .to_string_lossy()
                                    .to_string();
                                self.documents.push(IndexedDocument {
                                    source: relative,
                                    text,
                                });
                            }
                        }
                    }
                }
            }
        }
    }

    /// Build the full index from a campaign path: PDFs + markdown.
    pub fn build(campaign_path: &Path) -> Self {
        let mut index = Self::new();
        index.index_pdfs(campaign_path);
        index.index_markdown(campaign_path);
        index
    }
}

/// Extract text from a PDF file using pdf-extract.
fn extract_pdf_text(path: &Path) -> Result<String, String> {
    pdf_extract::extract_text(path).map_err(|e| format!("PDF extraction failed: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Create a minimal valid PDF with text content for testing.
    /// This generates a bare-bones PDF 1.4 file with a single page containing the given text.
    fn create_test_pdf(text: &str) -> Vec<u8> {
        // Minimal valid PDF structure with text content
        let content = format!("BT /F1 12 Tf 100 700 Td ({text}) Tj ET");
        let content_bytes = content.as_bytes();

        let mut pdf = Vec::new();

        // Header
        pdf.extend_from_slice(b"%PDF-1.4\n");

        // Object 1: Catalog
        let obj1_offset = pdf.len();
        pdf.extend_from_slice(b"1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n");

        // Object 2: Pages
        let obj2_offset = pdf.len();
        pdf.extend_from_slice(
            b"2 0 obj\n<< /Type /Pages /Kids [3 0 R] /Count 1 >>\nendobj\n",
        );

        // Object 3: Page
        let obj3_offset = pdf.len();
        pdf.extend_from_slice(b"3 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Contents 4 0 R /Resources << /Font << /F1 5 0 R >> >> >>\nendobj\n");

        // Object 4: Content stream
        let obj4_offset = pdf.len();
        let stream = format!(
            "4 0 obj\n<< /Length {} >>\nstream\n{}\nendstream\nendobj\n",
            content_bytes.len(),
            content
        );
        pdf.extend_from_slice(stream.as_bytes());

        // Object 5: Font
        let obj5_offset = pdf.len();
        pdf.extend_from_slice(b"5 0 obj\n<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>\nendobj\n");

        // Cross-reference table
        let xref_offset = pdf.len();
        pdf.extend_from_slice(b"xref\n0 6\n");
        pdf.extend_from_slice(b"0000000000 65535 f \n");
        pdf.extend_from_slice(format!("{:010} 00000 n \n", obj1_offset).as_bytes());
        pdf.extend_from_slice(format!("{:010} 00000 n \n", obj2_offset).as_bytes());
        pdf.extend_from_slice(format!("{:010} 00000 n \n", obj3_offset).as_bytes());
        pdf.extend_from_slice(format!("{:010} 00000 n \n", obj4_offset).as_bytes());
        pdf.extend_from_slice(format!("{:010} 00000 n \n", obj5_offset).as_bytes());

        // Trailer
        pdf.extend_from_slice(
            format!(
                "trailer\n<< /Size 6 /Root 1 0 R >>\nstartxref\n{xref_offset}\n%%EOF\n"
            )
            .as_bytes(),
        );

        pdf
    }

    #[test]
    fn test_extract_text_from_test_pdf() {
        let dir = tempfile::TempDir::new().unwrap();
        let pdf_path = dir.path().join("test.pdf");
        let pdf_data = create_test_pdf("The ancient dragon sleeps in the mountain");
        std::fs::write(&pdf_path, &pdf_data).unwrap();

        let result = extract_pdf_text(&pdf_path);
        assert!(result.is_ok(), "PDF extraction failed: {:?}", result.err());
        let text = result.unwrap();
        assert!(
            text.contains("ancient dragon"),
            "Extracted text should contain the PDF content. Got: {text:?}"
        );
    }

    #[test]
    fn test_index_pdfs_from_campaign() {
        let dir = tempfile::TempDir::new().unwrap();
        let docs_dir = dir.path().join("rules").join("docs");
        std::fs::create_dir_all(&docs_dir).unwrap();

        // Create two test PDFs
        std::fs::write(
            docs_dir.join("handbook.pdf"),
            create_test_pdf("Rules for combat and spellcasting"),
        )
        .unwrap();
        std::fs::write(
            docs_dir.join("bestiary.pdf"),
            create_test_pdf("The goblin is a small creature"),
        )
        .unwrap();
        // Non-PDF file should be ignored
        std::fs::write(docs_dir.join("notes.txt"), "not a pdf").unwrap();

        let mut index = SearchIndex::new();
        index.index_pdfs(dir.path());

        assert_eq!(index.documents.len(), 2, "Should index 2 PDF files");
        let sources: Vec<&str> = index.documents.iter().map(|d| d.source.as_str()).collect();
        assert!(
            sources.iter().any(|s| s.contains("handbook.pdf")),
            "Should index handbook.pdf. Sources: {sources:?}"
        );
        assert!(
            sources.iter().any(|s| s.contains("bestiary.pdf")),
            "Should index bestiary.pdf. Sources: {sources:?}"
        );
    }

    #[test]
    fn test_index_pdfs_missing_directory() {
        let dir = tempfile::TempDir::new().unwrap();
        // No rules/docs/ directory exists
        let mut index = SearchIndex::new();
        index.index_pdfs(dir.path());
        assert!(
            index.documents.is_empty(),
            "Missing docs dir should produce empty index"
        );
    }

    #[test]
    fn test_index_pdfs_empty_directory() {
        let dir = tempfile::TempDir::new().unwrap();
        let docs_dir = dir.path().join("rules").join("docs");
        std::fs::create_dir_all(&docs_dir).unwrap();

        let mut index = SearchIndex::new();
        index.index_pdfs(dir.path());
        assert!(
            index.documents.is_empty(),
            "Empty docs dir should produce empty index"
        );
    }

    #[test]
    fn test_extract_pdf_invalid_file() {
        let dir = tempfile::TempDir::new().unwrap();
        let bad_path = dir.path().join("not_a_pdf.pdf");
        std::fs::write(&bad_path, "this is not a pdf").unwrap();

        let result = extract_pdf_text(&bad_path);
        assert!(result.is_err(), "Invalid PDF should return error");
    }

    #[test]
    fn test_extract_pdf_nonexistent_file() {
        let result = extract_pdf_text(Path::new("/nonexistent/file.pdf"));
        assert!(result.is_err(), "Nonexistent file should return error");
    }

    // --- Markdown indexing tests ---

    #[test]
    fn test_index_markdown_from_campaign() {
        let dir = tempfile::TempDir::new().unwrap();

        // Create player with lore
        let player_dir = dir.path().join("players").join("thorin");
        std::fs::create_dir_all(&player_dir).unwrap();
        std::fs::write(
            player_dir.join("lore.md"),
            "# Thorin\nA dwarf warrior from the Iron Hills.",
        )
        .unwrap();

        // Create NPC with lore and dialogues
        let npc_dir = dir.path().join("npc").join("goblin_king");
        std::fs::create_dir_all(&npc_dir).unwrap();
        std::fs::write(
            npc_dir.join("lore.md"),
            "# Goblin King\nRules from his dark throne in the mountain.",
        )
        .unwrap();
        std::fs::write(
            npc_dir.join("dialogues.md"),
            "# Dialogues\n\"You dare enter my domain?\"",
        )
        .unwrap();

        let mut index = SearchIndex::new();
        index.index_markdown(dir.path());

        assert_eq!(
            index.documents.len(),
            3,
            "Should index 3 markdown files (1 player lore + 1 NPC lore + 1 NPC dialogues)"
        );

        let sources: Vec<&str> = index.documents.iter().map(|d| d.source.as_str()).collect();
        assert!(sources.iter().any(|s| s.contains("thorin") && s.contains("lore.md")));
        assert!(sources.iter().any(|s| s.contains("goblin_king") && s.contains("lore.md")));
        assert!(sources.iter().any(|s| s.contains("goblin_king") && s.contains("dialogues.md")));
    }

    #[test]
    fn test_index_markdown_missing_directories() {
        let dir = tempfile::TempDir::new().unwrap();
        // No players/ or npc/ directories
        let mut index = SearchIndex::new();
        index.index_markdown(dir.path());
        assert!(index.documents.is_empty());
    }

    #[test]
    fn test_index_markdown_empty_files_skipped() {
        let dir = tempfile::TempDir::new().unwrap();
        let player_dir = dir.path().join("players").join("empty");
        std::fs::create_dir_all(&player_dir).unwrap();
        std::fs::write(player_dir.join("lore.md"), "").unwrap();
        std::fs::write(player_dir.join("dialogues.md"), "  \n  ").unwrap();

        let mut index = SearchIndex::new();
        index.index_markdown(dir.path());
        assert!(
            index.documents.is_empty(),
            "Empty/whitespace-only files should be skipped"
        );
    }

    #[test]
    fn test_build_indexes_both_pdfs_and_markdown() {
        let dir = tempfile::TempDir::new().unwrap();

        // Add a PDF
        let docs_dir = dir.path().join("rules").join("docs");
        std::fs::create_dir_all(&docs_dir).unwrap();
        std::fs::write(
            docs_dir.join("rules.pdf"),
            create_test_pdf("Combat rules for the brave"),
        )
        .unwrap();

        // Add a markdown lore file
        let npc_dir = dir.path().join("npc").join("dragon");
        std::fs::create_dir_all(&npc_dir).unwrap();
        std::fs::write(
            npc_dir.join("lore.md"),
            "# Ancient Dragon\nSleeps beneath the mountain.",
        )
        .unwrap();

        let index = SearchIndex::build(dir.path());

        assert_eq!(
            index.documents.len(),
            2,
            "Should index 1 PDF + 1 markdown"
        );
        let sources: Vec<&str> = index.documents.iter().map(|d| d.source.as_str()).collect();
        assert!(sources.iter().any(|s| s.contains("rules.pdf")));
        assert!(sources.iter().any(|s| s.contains("lore.md")));
    }
}
