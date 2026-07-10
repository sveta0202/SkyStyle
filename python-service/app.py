import os

import requests
from flask import Flask, jsonify, render_template, request

app = Flask(__name__)
RUST_SERVICE_URL = os.environ.get("RUST_SERVICE_URL", "http://rust-service:8080")

@app.get("/")
def index():
    return render_template("index.html")

@app.get("/auth")
def auth():
    return render_template("login.html")

@app.get("/reg")
def reg():
    return render_template("registr.html")

@app.post("/collect-registration")
def collect_registration():
    data = request.get_json(silent=True) or {}
    registration = data.get("registration", {})

    payload = {
        "name": registration.get("user_name", "").strip(),
        "mail": registration.get("email", "").strip().lower(),
        "password": registration.get("password", ""),
    }

    try:
        resp = requests.post(
            f"{RUST_SERVICE_URL}/auth/register",
            json=payload,
            timeout=10,
        )
    except requests.RequestException as e:
        return jsonify({"ok": False, "error": f"нет связи с бэкендом: {e}"}), 502

    if resp.status_code >= 400:
        err = resp.json().get("error", "ошибка регистрации")
        return jsonify({"ok": False, "error": err}), resp.status_code

    return jsonify({
        "ok": True,
        "user": {
            "user_name": payload["name"],
            "email": payload["mail"],
        },
    })


@app.post("/login")
def login():
    data = request.get_json(silent=True) or {}

    payload = {
        "name": data.get("user_name", "").strip(),
        "mail": data.get("email", "").strip().lower(),
        "password": data.get("password", ""),
    }

    try:
        resp = requests.post(
            f"{RUST_SERVICE_URL}/auth/login",
            json=payload,
            timeout=10,
        )
    except requests.RequestException as e:
        return jsonify({"ok": False, "error": f"нет связи с бэкендом: {e}"}), 502

    if resp.status_code >= 400:
        err = resp.json().get("error", "ошибка входа")
        return jsonify({"ok": False, "error": err}), resp.status_code

    body = resp.json()
    return jsonify({
        "ok": True,
        "user": {
            "user_name": body.get("name"),
            "email": body.get("mail"),
            "id": str(body.get("id")),
        },
    })


if __name__ == "__main__":
    app.run(host="0.0.0.0", port=8000, debug=True)