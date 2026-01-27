# Azure Pipelines Configuration

This directory contains Azure Pipelines YAML configurations for the Flowtrace project.

## Available Pipelines

| Pipeline | File | Description | Trigger |
|----------|------|-------------|---------|
| **CI** | `../azure-pipelines.yml` | Main CI pipeline - tests, linting, builds | Push to main/develop, PRs |
| **Release Linux** | `release-linux.yml` | Build Tauri app for Linux (x64) | Manual/Release |
| **Release macOS** | `release-macos.yml` | Build Tauri app for macOS (ARM64 + Intel) | Manual/Release |
| **Release Windows** | `release-windows.yml` | Build Tauri app for Windows (x64) | Manual/Release |

## Setup Instructions

### 1. Create Pipelines in Azure DevOps

For each pipeline you want to use:

1. Go to **Azure DevOps** → **Pipelines** → **New Pipeline**
2. Select your repository (flowtrace)
3. Choose **"Existing Azure Pipelines YAML file"**
4. Select the appropriate YAML file:
   - For CI: select `azure-pipelines.yml` (root)
   - For releases: select from `.azure-pipelines/` folder
5. Click **Save** (or **Save and Run**)

### 2. Configure Pipeline Variables

Go to each release pipeline's settings and add the required variables:

#### Release Pipelines (Linux/macOS/Windows)
| Variable | Description | Secret |
|----------|-------------|--------|
| `TAURI_PRIVATE_KEY` | Tauri app signing private key | ✅ Yes |
| `TAURI_KEY_PASSWORD` | Password for the signing key | ✅ Yes |

To add variables:
1. Go to **Pipelines** → Select your pipeline → **Edit**
2. Click **Variables** (top right)
3. Add each variable with "Keep this value secret" checked

## Pipeline Features

### CI Pipeline
- **Frontend Tests**: Type checking, linting, building with Bun
- **SDK Tests**: Tests for JavaScript, Go, Rust, and Python SDKs
- **Build Check**: Full Rust workspace build with Clippy linting

### Release Pipelines
- Build Tauri desktop application for each platform
- Upload artifacts for download
- Signing support with Tauri keys

## Comparison with GitHub Actions

| GitHub Actions | Azure Pipelines |
|---------------|-----------------|
| `.github/workflows/ci.yml` | `azure-pipelines.yml` |
| `.github/workflows/release-linux.yml` | `.azure-pipelines/release-linux.yml` |
| `.github/workflows/release-mac.yml` | `.azure-pipelines/release-macos.yml` |
| `.github/workflows/release-windows.yml` | `.azure-pipelines/release-windows.yml` |
