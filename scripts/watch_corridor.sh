#!/usr/bin/env bash
# Watch real-time Waymesh corridor network events via SSE

NODE_URL="${WAYMESH_NODE_URL:-http://localhost:8080}"
echo "=========================================================="
echo "  Waymesh Network Observer - Listening to Corridor Events"
echo "  Node URL: ${NODE_URL}"
echo "=========================================================="
echo ""

curl -s -N "${NODE_URL}/api/v1/events/stream" | while read -r line; do
    if [[ $line == data:* ]]; then
        json_data="${line#data: }"
        event_type=$(echo "$json_data" | grep -o '"event":"[^"]*"' | cut -d'"' -f4)
        timestamp=$(echo "$json_data" | grep -o '"timestamp":"[^"]*"' | cut -d'"' -f4)
        
        echo -e "\033[1;36m[EVENT]\033[0m \033[1;33m${event_type}\033[0m \033[0;90m(${timestamp})\033[0m"
        echo "$json_data"
        echo "----------------------------------------------------------"
    fi
done
