# Security Policy

## Supported Versions

| Version | Supported          |
|---------|--------------------|
| 0.x     | :white_check_mark: |

Only the latest release is supported with security updates.

## Reporting a Vulnerability

**Please do not open a public issue for security vulnerabilities.**

If you discover a security vulnerability in kora, please report it through GitHub's private vulnerability reporting feature:

1. Go to the [Security tab](https://github.com/kafkade/kora/security) of this repository
2. Click **"Report a vulnerability"**
3. Fill out the form with details about the vulnerability

### What to expect

- **Acknowledgment**: We will acknowledge your report within **48 hours**
- **Assessment**: We will assess the severity and impact within **7 days**
- **Resolution**: We aim to release a fix within **90 days** of the initial report, depending on complexity
- **Disclosure**: We will coordinate disclosure timing with you

### What to include

- A description of the vulnerability
- Steps to reproduce the issue
- The potential impact
- Any suggested fixes (if you have them)

### Scope

Security issues in kora may include:

- Credential storage vulnerabilities (OAuth tokens, API keys)
- Path traversal in file browser or local provider
- Memory safety issues in the audio pipeline
- Malicious audio file processing (buffer overflow, denial of service)
- Plugin sandbox escapes (when the plugin system is implemented)

## Thank You

We appreciate responsible disclosure and will credit reporters (with their permission) in our release notes and changelog.
