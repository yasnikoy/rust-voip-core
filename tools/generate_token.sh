#!/bin/bash
# Token Generator for Screen Share Test
# Usage: ./generate_token.sh [room_name] [identity]

ROOM=${1:-test-room}
IDENTITY=${2:-web-tester-$(date +%s)}

echo "🔧 LiveKit Token Generator"
echo "=========================="
echo "Room: $ROOM"
echo "Identity: $IDENTITY"
echo ""

# Generate token using livekit-server container
TOKEN=$(docker run --rm livekit/livekit-server:latest create-join-token \
    --room "$ROOM" \
    --identity "$IDENTITY" \
    --keys "change_me: change_me" 2>/dev/null | grep "Token:" | cut -d' ' -f2)

if [ -z "$TOKEN" ]; then
    echo "❌ Token oluşturulamadı!"
    echo "   LiveKit container çalıştığından emin olun:"
    echo "   docker compose up -d"
    exit 1
fi

echo "✅ Token oluşturuldu!"
echo ""
echo "Token:"
echo "$TOKEN"
echo ""
echo "📋 Bu token'ı kopyalayıp screen_share_test.html'e yapıştırın."
echo ""
echo "🌐 Tarayıcıda açmak için:"
echo "   file://$(pwd)/tools/screen_share_test.html"
