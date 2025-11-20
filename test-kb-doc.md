# Test Knowledge Base

This is a test document for the knowledge base system.

## Overview

The Guided Agent CLI provides a complete knowledge management system using local-first RAG (Retrieval-Augmented Generation). 

Key features:
- SQLite-backed vector storage
- Embeddings for semantic search
- Support for multiple file formats (Markdown, HTML, code)
- Chunk-based text processing with configurable overlap

## Usage

You can learn from files using:
```bash
guided knowledge learn my-base --path ./docs
```

Query the knowledge base:
```bash
guided knowledge ask my-base "What is RAG?"
```

View statistics:
```bash
guided knowledge stats my-base
```

Clean the knowledge base:
```bash
guided knowledge clean my-base
```
