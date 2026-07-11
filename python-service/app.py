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


@app.get("/home")
def home():
    return render_template("home.html", weather=None, outfits=[])


@app.get("/reg")
def reg():
    return render_template("registr.html")


@app.get("/outfits-page")
def outfits_page():
    return render_template("outfits.html")


@app.get("/generate")
def generate():
    return render_template("generate.html")


@app.get("/profile")
def profile():
    return render_template("profile.html")


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

    try:
        body = resp.json()
    except ValueError:
        body = {"ok": False, "error": resp.text}

    if resp.status_code >= 400:
        return jsonify({
            "ok": False,
            "error": body.get("error", "ошибка регистрации"),
        }), resp.status_code

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

    try:
        body = resp.json()
    except ValueError:
        body = {"ok": False, "error": resp.text}

    if resp.status_code >= 400:
        return jsonify({
            "ok": False,
            "error": body.get("error", "ошибка входа"),
        }), resp.status_code

    return jsonify({
        "ok": True,
        "user": {
            "user_name": body.get("name"),
            "email": body.get("mail"),
            "id": str(body.get("id")),
        },
    })


def _proxy(method, path, *, json_body=None, params=None, timeout=30):
    try:
        resp = requests.request(
            method,
            f"{RUST_SERVICE_URL}{path}",
            json=json_body,
            params=params,
            timeout=timeout,
        )
    except requests.RequestException as e:
        return {"ok": False, "error": f"нет связи с бэкендом: {e}"}, 502

    try:
        payload = resp.json()
    except ValueError:
        payload = {"ok": False, "error": resp.text}

    return payload, resp.status_code


@app.get("/wardrobe")
def wardrobe_get():
    user_id = request.args.get("user_id", "").strip()
    payload, status = _proxy("GET", "/wardrobe", params={"user_id": user_id})
    return jsonify(payload), status


@app.post("/wardrobe")
def wardrobe_post():
    data = request.get_json(silent=True) or {}
    payload, status = _proxy("POST", "/wardrobe", json_body=data)
    return jsonify(payload), status


@app.delete("/wardrobe")
def wardrobe_delete():
    data = request.get_json(silent=True) or {}
    payload, status = _proxy("DELETE", "/wardrobe", json_body=data)
    return jsonify(payload), status


@app.get("/weather")
def weather():
    city = request.args.get("city", "").strip()
    payload, status = _proxy("GET", "/weather", params={"city": city})
    return jsonify(payload), status


@app.get("/outfits")
def outfits_api():
    user_id = request.args.get("user_id", "").strip()
    payload, status = _proxy("GET", "/outfits", params={"user_id": user_id})
    return jsonify(payload), status


@app.post("/generate-outfit")
def generate_outfit():
    data = request.get_json(silent=True) or {}

    payload = {
        "user_id": data.get("user_id"),
        "city": (data.get("city") or "").strip(),
        "goal": (data.get("goal") or "").strip(),
        "tone": (data.get("tone") or "").strip(),
    }

    if not payload["user_id"]:
        return jsonify({"ok": False, "error": "user_id обязателен"}), 400

    if not payload["city"]:
        return jsonify({"ok": False, "error": "city обязателен"}), 400

    body, status = _proxy("POST", "/outfits/generate", json_body=payload, timeout=90)
    return jsonify(body), status


if __name__ == "__main__":
    app.run(host="0.0.0.0", port=8000, debug=True)