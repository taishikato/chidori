# chidori

A fast Rust-built web-to-Markdown fetcher for coding agents.

```bash
npx chidori-fetch https://example.com
```

`chidori` fetches a URL, extracts the main readable content, removes page noise, and prints Markdown to stdout by default. Logs and errors go to stderr so agents and shell scripts can safely pipe stdout into prompts, files, or other tools.

## Usage

```bash
chidori https://example.com
chidori https://example.com --json
chidori https://example.com --output article.md
chidori https://example.com --max-chars 20000
chidori https://example.com --lang ja
chidori https://example.com --no-images
```

## Why

Coding agents often need web pages as clean Markdown. `chidori` is built in Rust for fast CLI startup and predictable shell behavior.

## Status

This project is an MVP. It focuses on URL input, Markdown output, metadata JSON, and generic content extraction.
