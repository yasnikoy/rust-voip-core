#!/bin/bash
# LiveKit Token Generator
# Generates both publisher and viewer tokens
#
# Usage: ./generate_token.sh [room_name]

ROOM=${1:-test-room}
TIMESTAMP=$(date +%s)

echo "🔧 LiveKit Token Generator"
echo "=========================="
echo "Room: $ROOM"
echo ""

# Publisher Token (can publish and subscribe)
echo "📤 Publisher Token (ekran paylaşımı için):"
echo "   Identity: publisher-$TIMESTAMP"
PUBLISHER_TOKEN=$(docker run --rm livekit/livekit-server:latest create-join-token \
    --room "$ROOM" \
    --identity "publisher-$TIMESTAMP" \
    --keys "change_me: change_me" 2>/dev/null | grep "Token:" | cut -d' ' -f2)

if [ -z "$PUBLISHER_TOKEN" ]; then
    echo "❌ Token oluşturulamadı!"
    echo "   LiveKit container çalıştığından emin olun:"
    echo "   docker compose up -d"
    exit 1
fi

echo "$PUBLISHER_TOKEN"
echo ""

# Viewer Token (can only subscribe)
echo "📥 Viewer Token (izlemek için):"
echo "   Identity: viewer-$TIMESTAMP"
VIEWER_TOKEN=$(docker run --rm livekit/livekit-server:latest create-join-token \
    --room "$ROOM" \
    --identity "viewer-$TIMESTAMP" \
    --recorder \
    --keys "change_me: change_me" 2>/dev/null | grep "Token:" | cut -d' ' -f2)

echo "$VIEWER_TOKEN"
echo ""
echo "✅ Tokenlar oluşturuldu!"
echo ""
echo "🌐 Test aracını açmak için:"
echo "   xdg-open tools/screen_share_test.html  # Linux"
echo "   open tools/screen_share_test.html      # macOS"
echo "   start tools/screen_share_test.html     # Windows"
