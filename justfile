# Rustory justfile

# Run the project (debug)
run:
    cp -r sample/ target/debug
    cp -r assets/ target/debug
    cargo run

# Build for Linux (default)
build: (_build "x86_64-unknown-linux-gnu")

# Build for macOS
build-mac: (_build "x86_64-apple-darwin")

# Build for Windows
build-win: (_build "x86_64-pc-windows-gnu")

# Launch Ralph autonomous dev loop
ralph:
    ./ralph/ralph.sh

# Launch Ralph after a delay (e.g., `just ralph-delay 3h50m`)
ralph-delay duration:
    echo "Ralph will launch in {{duration}}..."
    python3 -c "import re,time,sys;d=sys.argv[1];s=sum(int(v)*{'h':3600,'m':60,'s':1}[u] for v,u in re.findall(r'(\d+)([hms])',d));print(f'Sleeping {s}s...');time.sleep(s)" {{duration}}
    ./ralph/ralph.sh

# Watch Ralph logs (formatted)
log:
    tail -f ralph/ralph.log | jq -Rr 'try (fromjson | if .type == "assistant" then .message.content[]? | if .type == "text" then "💬 \(.text)" elif .type == "tool_use" then "🔧 \(.name)(\(.input | keys | join(", ")))" else empty end elif .type == "user" then .message.content[]? | if .type == "tool_result" then (if .is_error then "❌ \(.content[:150])" else "✅ \(.content[:150])" end) else empty end else empty end) catch empty'

_build target:
    rustup target add {{target}}
    cargo build --release --target {{target}} --target-dir .
    rm -rdf release/
    mv {{target}}/release/* {{target}}/
    rm -rdf {{target}}/release/
    cp -r sample/ {{target}}/
    cp -r assets/ {{target}}/
