# gemini-multimodal-mcp

MCP server that gives visionless LLMs Gemini's eyes, ears, and video comprehension via `agy` CLI.

## Quick start

### 1. Install `agy`

Download from <https://antigravity.google> and follow its setup instructions to install and authenticate.

### 2. Install FFmpeg

```sh
# macOS
brew install ffmpeg

# Debian / Ubuntu
sudo apt install libavformat-dev libavcodec-dev libavutil-dev libswscale-dev

# Fedora
sudo dnf install ffmpeg-devel

# Arch
sudo pacman -S ffmpeg
```

### 3. Build

Requires Rust 1.85+.

```sh
git clone https://github.com/yuygfgg/gemini-multimodal-mcp
cd gemini-multimodal-mcp
cargo build --release
```

The binary is at `target/release/gemini-multimodal-mcp`

## Connect to your MCP client

Add the server to your client's config and restart. You can optionally configure a default model for the server to use (e.g. `--model "Gemini 3.5 Flash (Medium)"`) so that client tool calls do not need to specify the `model` parameter on every request.

**opencode** (`opencode.json`):

```jsonc
{
  "mcpServers": {
    "gemini-vision": {
      "type": "local",
      "command": ["/path/to/gemini-multimodal-mcp", "--model", "Gemini 3.5 Flash (Medium)"]
    }
  }
}
```

**Other clients** — just point `command` at the binary path and append model options if desired.

## What you get

Once connected, your agent gains three tools:

- **`describe_image`** — exhaustive structured description of any image (photos, charts, screenshots, documents, memes)
- **`describe_video`** — temporal walkthrough covering people, objects, on-screen text, audio, lighting, and mood
- **`describe_audio`** — speakers, languages, tone, speech content, music, sound effects, and ambience

All tools accept local file paths and `data:` URIs. Videos longer than 5 minutes and audio longer than 30 minutes require an explicit confirmation flag to protect your quota.

## License

Apache 2.0, see [LICENSE](LICENSE) for details.
