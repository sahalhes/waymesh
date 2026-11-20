"""
title: Waymesh Open Protocol Tool
author: Waymesh Maintainers
description: Connects OpenWebUI directly to an independent local Waymesh node for ridesharing and parcel hitchhiking. Enforces strict gender separation for passenger rides (male and female are never matched together).
version: 0.2.0
"""

import os
import json
import urllib.request
import secrets
from typing import Optional, Dict, Any

class Tools:
    def __init__(self):
        self.node_url = os.environ.get(
            "WAYMESH_NODE_URL",
            os.environ.get("SERVER_URL", "http://localhost:8080"),
        ).rstrip("/")

    def _post(self, path: str, payload: Dict[str, Any]) -> str:
        req = urllib.request.Request(
            f"{self.node_url}{path}",
            data=json.dumps(payload).encode("utf-8"),
            headers={"Content-Type": "application/json"},
        )
        try:
            with urllib.request.urlopen(req) as response:
                return response.read().decode("utf-8")
        except urllib.error.HTTPError as e:
            return json.dumps({"error": f"HTTP {e.code}: {e.read().decode('utf-8')}"})
        except Exception as e:
            return json.dumps({"error": str(e)})

    def _get(self, path: str) -> str:
        req = urllib.request.Request(
            f"{self.node_url}{path}",
            headers={"Content-Type": "application/json"},
        )
        try:
            with urllib.request.urlopen(req) as response:
                return response.read().decode("utf-8")
        except urllib.error.HTTPError as e:
            return json.dumps({"error": f"HTTP {e.code}: {e.read().decode('utf-8')}"})
        except Exception as e:
            return json.dumps({"error": str(e)})

    def register_identity(
        self,
        display_name: str,
        gender: str = "other",
        public_key_ed25519: Optional[str] = None,
    ) -> str:
        """
        Register a new identity on the Waymesh node.
        :param display_name: Name or alias of user
        :param gender: 'male', 'female', or 'other' (enforced in ride matching)
        :param public_key_ed25519: Hex-encoded Ed25519 public key (generated if omitted)
        """
        if not public_key_ed25519:
            public_key_ed25519 = secrets.token_hex(32)
        payload = {
            "display_name": display_name,
            "gender": gender.lower(),
            "public_key_ed25519": public_key_ed25519,
        }
        return self._post("/api/v1/identities", payload)

    def publish_journey(
        self,
        origin_name: str,
        destination_name: str,
        departure_time: str,
        origin_lat: float = 0.0,
        origin_lng: float = 0.0,
        destination_lat: float = 0.0,
        destination_lng: float = 0.0,
        driver_id: Optional[str] = None,
        driver_name: Optional[str] = None,
        driver_gender: Optional[str] = None,
        seats: int = 3,
        cargo_kg: float = 10.0,
        cost_per_seat: int = 8000,
    ) -> str:
        """
        Publish a planned driver or commuter journey on the Waymesh network.
        :param origin_name: Human readable departure name (landmarks auto-resolved if coords omitted)
        :param destination_name: Destination landmark or station name
        :param departure_time: ISO-8601 departure timestamp (e.g. 2026-11-18T17:00:00Z)
        :param driver_id: Optional UUID of the driver
        :param driver_name: Optional display name of driver (auto-provisions identity)
        :param driver_gender: 'male', 'female', or 'other' (strictly enforced in ride matching)
        :param seats: Number of empty passenger seats available
        :param cargo_kg: Trunk/cargo capacity in kilograms
        :param cost_per_seat: Suggested cost share per seat in minor units (8000 = ₹80.00, settled directly between parties privately)
        """
        payload = {
            "origin_name": origin_name,
            "origin_lat": origin_lat,
            "origin_lng": origin_lng,
            "destination_name": destination_name,
            "destination_lat": destination_lat,
            "destination_lng": destination_lng,
            "departure_time": departure_time,
            "seats": seats,
            "cargo_kg": cargo_kg,
            "suggested_cost_per_seat": cost_per_seat,
        }
        if driver_id:
            payload["driver_id"] = driver_id
        if driver_name:
            payload["driver_name"] = driver_name
        if driver_gender:
            payload["driver_gender"] = driver_gender.lower()
        return self._post("/api/v1/journeys", payload)

    def publish_ride_request(
        self,
        origin_name: str,
        destination_name: str,
        desired_time: str,
        passenger_gender: str,
        passenger_name: Optional[str] = None,
        passenger_id: Optional[str] = None,
        origin_lat: float = 0.0,
        origin_lng: float = 0.0,
        destination_lat: float = 0.0,
        destination_lng: float = 0.0,
        seats: int = 1,
        max_cost_share: Optional[int] = None,
    ) -> str:
        """
        Publish a passenger ride request (commuter demand / wanted ride) when no matching driver ride is available.
        Drivers heading along your route can see your request and offer/fulfill it.
        Strictly enforces gender separation (males match only with males, females only with females).
        """
        payload = {
            "origin_name": origin_name,
            "origin_lat": origin_lat,
            "origin_lng": origin_lng,
            "destination_name": destination_name,
            "destination_lat": destination_lat,
            "destination_lng": destination_lng,
            "desired_time": desired_time,
            "passenger_gender": passenger_gender.lower(),
            "seats": seats,
        }
        if passenger_id:
            payload["passenger_id"] = passenger_id
        if passenger_name:
            payload["passenger_name"] = passenger_name
        if max_cost_share is not None:
            payload["max_cost_share"] = max_cost_share
        return self._post("/api/v1/ride-requests", payload)

    def search_ride_requests(
        self,
        origin_name: Optional[str] = None,
        origin_lat: float = 0.0,
        origin_lng: float = 0.0,
        destination_name: Optional[str] = None,
        destination_lat: float = 0.0,
        destination_lng: float = 0.0,
        departure_time: str = "now",
        driver_gender: Optional[str] = None,
        max_detour_minutes: int = 15,
    ) -> str:
        """
        Search open passenger ride requests along a corridor for drivers looking to offer a ride.
        Landmark names (origin_name, destination_name) are auto-resolved if coords omitted.
        """
        payload = {
            "origin_lat": origin_lat,
            "origin_lng": origin_lng,
            "destination_lat": destination_lat,
            "destination_lng": destination_lng,
            "departure_time": departure_time,
            "max_detour_minutes": max_detour_minutes,
        }
        if origin_name:
            payload["origin_name"] = origin_name
        if destination_name:
            payload["destination_name"] = destination_name
        if driver_gender:
            payload["driver_gender"] = driver_gender.lower()
        return self._post("/api/v1/ride-requests/search", payload)

    def get_ride_request_status(self, request_id: str) -> str:
        """
        Check the status of a published passenger ride request.
        Returns whether it is Published, Matched (with driver info, booking details, and 6-digit pickup OTP), or Cancelled.
        """
        return self._get(f"/api/v1/ride-requests/{request_id}")

    def cancel_ride_request(self, request_id: str) -> str:
        """
        Cancel an active passenger ride request.
        """
        return self._post(f"/api/v1/ride-requests/{request_id}/cancel", {})

    def accept_ride_request(
        self,
        request_id: str,
        driver_name: Optional[str] = None,
        driver_gender: Optional[str] = None,
        driver_id: Optional[str] = None,
        journey_id: Optional[str] = None,
        agreed_cost_share: Optional[int] = None,
    ) -> str:
        """
        Driver accepts/fulfills an open passenger ride request, reserving seats and generating the secret 6-digit pickup OTP.
        """
        payload = {}
        if driver_id:
            payload["driver_id"] = driver_id
        if driver_name:
            payload["driver_name"] = driver_name
        if driver_gender:
            payload["driver_gender"] = driver_gender.lower()
        if journey_id:
            payload["journey_id"] = journey_id
        if agreed_cost_share is not None:
            payload["agreed_cost_share"] = agreed_cost_share
        return self._post(f"/api/v1/ride-requests/{request_id}/accept", payload)

    def search_matches(
        self,
        pickup_name: Optional[str] = None,
        pickup_lat: float = 0.0,
        pickup_lng: float = 0.0,
        dropoff_name: Optional[str] = None,
        dropoff_lat: float = 0.0,
        dropoff_lng: float = 0.0,
        desired_time: str = "now",
        demand_type: str = "passenger",
        passenger_gender: Optional[str] = None,
        passenger_id: Optional[str] = None,
        seats: int = 1,
        cargo_kg: float = 0.0,
    ) -> str:
        """
        Search for active journeys matching a ride or parcel route along the corridor.
        NOTE: Male and female riders are never matched together for passenger rides.
        :param pickup_name: Optional departure landmark (e.g. 'CET')
        :param pickup_lat: Latitude of pickup point (auto-resolved from pickup_name if 0)
        :param pickup_lng: Longitude of pickup point
        :param dropoff_name: Optional destination landmark (e.g. 'TVM Central Railway Station')
        :param dropoff_lat: Latitude of destination (auto-resolved from dropoff_name if 0)
        :param dropoff_lng: Longitude of destination
        :param desired_time: Desired travel time (ISO-8601 or relative like '5 pm', 'now')
        :param demand_type: 'passenger' or 'parcel'
        :param passenger_gender: 'male', 'female', or 'other' (prohibits cross-gender matching)
        :param passenger_id: Optional UUID of passenger
        """
        payload = {
            "pickup_lat": pickup_lat,
            "pickup_lng": pickup_lng,
            "dropoff_lat": dropoff_lat,
            "dropoff_lng": dropoff_lng,
            "desired_time": desired_time,
            "demand_type": demand_type,
            "seats": seats,
            "cargo_kg": cargo_kg,
        }
        if pickup_name:
            payload["pickup_name"] = pickup_name
        if dropoff_name:
            payload["dropoff_name"] = dropoff_name
        if passenger_gender:
            payload["passenger_gender"] = passenger_gender.lower()
        if passenger_id:
            payload["passenger_id"] = passenger_id
        return self._post("/api/v1/matches/search", payload)

    def create_booking(
        self,
        journey_id: str,
        pickup_name: str,
        pickup_lat: float,
        pickup_lng: float,
        dropoff_name: str,
        dropoff_lat: float,
        dropoff_lng: float,
        agreed_cost_share: int = 8000,
        passenger_id: Optional[str] = None,
        passenger_name: Optional[str] = None,
        passenger_gender: Optional[str] = None,
        seats: int = 1,
    ) -> str:
        """
        Request a seat on a journey. Zero-UUID support: passenger_id is optional; if omitted, identity is auto-created using passenger_name.
        Strict gender verification: male and female pairings return 403 Forbidden.
        """
        payload = {
            "journey_id": journey_id,
            "pickup_name": pickup_name,
            "pickup_lat": pickup_lat,
            "pickup_lng": pickup_lng,
            "dropoff_name": dropoff_name,
            "dropoff_lat": dropoff_lat,
            "dropoff_lng": dropoff_lng,
            "agreed_cost_share": agreed_cost_share,
            "seats": seats,
        }
        if passenger_id:
            payload["passenger_id"] = passenger_id
        if passenger_name:
            payload["passenger_name"] = passenger_name
        if passenger_gender:
            payload["passenger_gender"] = passenger_gender.lower()
        return self._post("/api/v1/bookings", payload)

    def request_parcel_delivery(
        self,
        recipient_name: str,
        recipient_phone: str,
        pickup_address: str,
        dropoff_address: str,
        weight_kg: float,
        contents: str,
        deadline_time: str,
        pickup_lat: float = 0.0,
        pickup_lng: float = 0.0,
        dropoff_lat: float = 0.0,
        dropoff_lng: float = 0.0,
        sender_id: Optional[str] = None,
        sender_name: Optional[str] = None,
        compensation: int = 10000,
    ) -> str:
        """
        Publish a request for a peer passenger to carry a package along their journey.
        """
        payload = {
            "recipient_name": recipient_name,
            "recipient_phone": recipient_phone,
            "pickup_address": pickup_address,
            "pickup_lat": pickup_lat,
            "pickup_lng": pickup_lng,
            "dropoff_address": dropoff_address,
            "dropoff_lat": dropoff_lat,
            "dropoff_lng": dropoff_lng,
            "weight_kg": weight_kg,
            "contents": contents,
            "deadline_time": deadline_time,
            "compensation": compensation,
        }
        if sender_id:
            payload["sender_id"] = sender_id
        if sender_name:
            payload["sender_name"] = sender_name
        return self._post("/api/v1/parcels", payload)

    def verify_handover(
        self,
        entity_id: str,
        otp: str,
        entity_type: str = "booking",
        action: str = "pickup",
    ) -> str:
        """
        Verify physical boarding or parcel delivery using the 6-digit OTP code.
        """
        clean_otp = "".join(c for c in otp if c.isdigit())
        if entity_type == "parcel":
            endpoint = f"/api/v1/parcels/{entity_id}/deliver" if action == "deliver" else f"/api/v1/parcels/{entity_id}/pickup"
        else:
            endpoint = f"/api/v1/bookings/{entity_id}/verify-pickup"
        return self._post(endpoint, {"otp": clean_otp})
