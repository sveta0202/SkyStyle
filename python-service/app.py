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
        body = {"error": resp.text}

    if resp.status_code >= 400:
        err = body.get("error", "ошибка регистрации")
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

    try:
        body = resp.json()
    except ValueError:
        body = {"error": resp.text}

    if resp.status_code >= 400:
        err = body.get("error", "ошибка входа")
        return jsonify({"ok": False, "error": err}), resp.status_code

    return jsonify({
        "ok": True,
        "user": {
            "user_name": body.get("name"),
            "email": body.get("mail"),
            "id": str(body.get("id")),
        },
    })


def _proxy(method, path, *, json_body=None, params=None):
    try:
        resp = requests.request(
            method,
            f"{RUST_SERVICE_URL}{path}",
            json=json_body,
            params=params,
            timeout=30,
        )
    except requests.RequestException as e:
        return jsonify({"ok": False, "error": f"нет связи с бэкендом: {e}"}), 502

    try:
        payload = resp.json()
    except ValueError:
        payload = {"error": resp.text}

    return jsonify(payload), resp.status_code


@app.get("/wardrobe")
def wardrobe_get():
    user_id = request.args.get("user_id", "").strip()
    return _proxy("GET", "/wardrobe", params={"user_id": user_id})


@app.post("/wardrobe")
def wardrobe_post():
    data = request.get_json(silent=True) or {}
    return _proxy("POST", "/wardrobe", json_body=data)


@app.delete("/wardrobe")
def wardrobe_delete():
    data = request.get_json(silent=True) or {}
    return _proxy("DELETE", "/wardrobe", json_body=data)


@app.get("/weather")
def weather():
    city = request.args.get("city", "").strip()
    return _proxy("GET", "/weather", params={"city": city})


@app.get("/outfits")
def outfits_api():
    user_id = request.args.get("user_id", "").strip()
    return _proxy("GET", "/outfits", params={"user_id": user_id})


@app.post("/generate-outfit")
def generate_outfit():
    data = request.get_json(silent=True) or {}
    return _proxy("POST", "/outfits/generate", json_body=data)


if __name__ == "__main__":
    app.run(host="0.0.0.0", port=8000, debug=True)