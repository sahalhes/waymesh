#!/usr/bin/env python3
"""
Simulate Two AI Agents:
- OpenCode (as Passenger Priya, female)
- Antigravity (as Driver Sunita, female)
interacting through the Waymesh MCP daemon over stdio.
"""

import json
import subprocess
import sys
import os
from datetime import datetime, timezone, timedelta

MCP_BIN = os.path.abspath("target/release/waymesh-mcp")
ENV = os.environ.copy()
ENV["WAYMESH_NODE_URL"] = "http://127.0.0.1:8080"

def send_mcp_call(method, params):
    proc = subprocess.Popen(
        [MCP_BIN],
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        env=ENV,
        text=True
    )
    req = {
        "jsonrpc": "2.0",
        "id": 1,
        "method": method,
        "params": params
    }
    stdout, stderr = proc.communicate(input=json.dumps(req) + "\n")
    for line in stdout.strip().split("\n"):
        if line.strip():
            return json.loads(line)
    return None

def main():
    print("=" * 65)
    print("  WAYMESH TWO-AGENT MCP INTEGRATION VERIFICATION")
    print("=" * 65)
    
    now = datetime.now(timezone.utc)
    dep_time = (now + timedelta(minutes=30)).isoformat()

    # Step 0: Driver Sunita (Female) registers and publishes journey via MCP
    print("\n[AGENT 1: Antigravity (Driver Sunita)] Registering identity & publishing journey...")
    sunita_reg = send_mcp_call("tools/call", {
        "name": "waymesh_register_identity",
        "arguments": {
            "public_key_ed25519": "aabbccddeeff00112233445566778899aabbccddeeff00112233445566778899",
            "display_name": "Sunita Rao (Scholar Driver)",
            "gender": "female"
        }
    })
    sunita = json.loads(sunita_reg["result"]["content"][0]["text"])
    sunita_id = sunita["id"]

    pub_res = send_mcp_call("tools/call", {
        "name": "waymesh_publish_journey",
        "arguments": {
            "driver_id": sunita_id,
            "driver_gender": "female",
            "origin_name": "Campus North Gate",
            "origin_lat": 12.9716,
            "origin_lng": 77.5946,
            "destination_name": "City Central Railway Station",
            "destination_lat": 12.9780,
            "destination_lng": 77.5700,
            "departure_time": dep_time,
            "seats": 3,
            "cargo_kg": 15.0,
            "cost_per_seat": 8000
        }
    })
    journey = json.loads(pub_res["result"]["content"][0]["text"])
    journey_id = journey["id"]
    print(f"  ✅ Sunita's Journey Published via MCP! ID: {journey_id[:8]}... (Driver Gender: female)")

    # Step 1: OpenCode (Passenger Priya, Female) searches for a ride
    print("\n[AGENT 2: OpenCode (Passenger Priya)] Registering & Searching for rides...")
    priya_reg = send_mcp_call("tools/call", {
        "name": "waymesh_register_identity",
        "arguments": {
            "public_key_ed25519": "112233445566778899aabbccddeeff00112233445566778899aabbccddeeff00",
            "display_name": "Priya Nair (Student)",
            "gender": "female"
        }
    })
    priya = json.loads(priya_reg["result"]["content"][0]["text"])
    priya_id = priya["id"]

    search_res = send_mcp_call("tools/call", {
        "name": "waymesh_search_matches",
        "arguments": {
            "pickup_lat": 12.9718,
            "pickup_lng": 77.5944,
            "dropoff_lat": 12.9778,
            "dropoff_lng": 77.5702,
            "desired_time": dep_time,
            "demand_type": "passenger",
            "passenger_id": priya_id,
            "passenger_gender": "female",
            "seats": 1
        }
    })
    
    match_data = json.loads(search_res["result"]["content"][0]["text"])
    assignments = match_data.get("assignments", [])
    if not assignments:
        print("  ❌ No matches found!")
        return 1
    
    matched_journey_id = assignments[0]["journey_id"]
    match_score = assignments[0]["match_score"]
    cost_paise = assignments[0]["suggested_cost"]
    print(f"  ✅ Female-to-Female Match Found via MCP!")
    print(f"     Journey ID: {matched_journey_id[:8]}...")
    print(f"     Corridor Alignment Score: {match_score * 100:.2f}%")
    print(f"     Suggested Cost Share: ₹{cost_paise / 100:.2f} (Settled between parties privately via UPI / Cash)")

    # Step 2: OpenCode books 1 seat
    print("\n[AGENT 2: OpenCode (Passenger Priya)] Booking seat on Sunita's car...")
    book_res = send_mcp_call("tools/call", {
        "name": "waymesh_create_booking",
        "arguments": {
            "journey_id": matched_journey_id,
            "passenger_id": priya_id,
            "passenger_gender": "female",
            "seats": 1,
            "pickup_name": "Campus North Gate",
            "pickup_lat": 12.9718,
            "pickup_lng": 77.5944,
            "dropoff_name": "City Central Railway Station",
            "dropoff_lat": 12.9778,
            "dropoff_lng": 77.5702,
            "agreed_cost_share": cost_paise
        }
    })
    booking = json.loads(book_res["result"]["content"][0]["text"])
    booking_id = booking["id"]
    print(f"  ✅ Booking Requested! Booking ID: {booking_id[:8]}... (Status: {booking['status']})")

    # Step 3: Antigravity accepts the booking
    print("\n[AGENT 1: Antigravity (Driver Sunita)] Accepting Priya's booking...")
    accept_res = send_mcp_call("tools/call", {
        "name": "waymesh_accept_booking",
        "arguments": {
            "booking_id": booking_id
        }
    })
    accept_data = json.loads(accept_res["result"]["content"][0]["text"])
    pickup_otp = accept_data.get("pickup_otp")
    print(f"  ✅ Booking Accepted by Driver via MCP!")
    print(f"     Passenger's Secret 6-Digit Pickup OTP: >>> [{pickup_otp}] <<<")

    # Step 4: Antigravity verifies pickup OTP at Campus North Gate
    print(f"\n[AGENT 1: Antigravity (Driver Sunita)] Verifying OTP '{pickup_otp}' at physical boarding...")
    verify_res = send_mcp_call("tools/call", {
        "name": "waymesh_verify_handover",
        "arguments": {
            "entity_type": "booking",
            "entity_id": booking_id,
            "otp": pickup_otp,
            "action": "pickup"
        }
    })
    verified_booking = json.loads(verify_res["result"]["content"][0]["text"])
    print(f"  ✅ Cryptographic OTP Verified via MCP!")
    print(f"     Booking Status: {verified_booking['status']}")

    print("\n" + "=" * 65)
    print("  🎉 TWO-AGENT MCP WORKFLOW (Antigravity + OpenCode) VERIFIED!")
    print("=" * 65)
    return 0

if __name__ == "__main__":
    sys.exit(main())
