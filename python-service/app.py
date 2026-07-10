import os
import requests
from flask import Flask, render_template, request, jsonify

app = Flask(__name__)

# Адрес Rust-микросервиса. В Docker — имя контейнера "rust-service",
# локально — localhost. Берём из переменной окружения.
RUST_SERVICE_URL = os.getenv("RUST_SERVICE_URL", "http://localhost:8080")

# Таймаут обращения к Rust (секунды)
RUST_TIMEOUT = 5


# ─── Страницы (фронтенд) ───

@app.get("/")
@app.get("/auth")
def index():
    # Лендинг из templates/index.html
    return render_template("index.html")


# ─── API: проксируем авторизацию в Rust-микросервис ───
# Этим Flask связан с Rust — браузер стучится в Python, а Python в Rust.

@app.post("/register")
def register():
    payload = {
        "name": request.form.get("name", "").strip(),
        "mail": request.form.get("mail", "").strip().lower(),
        "password": request.form.get("password", ""),
    }

    try:
        resp = requests.post(
            f"{RUST_SERVICE_URL}/auth/register",
            json=payload,
            timeout=RUST_TIMEOUT,
        )
    except requests.exceptions.ConnectionError:
        return jsonify({"error": "сервис данных недоступен", "code": "SERVICE_UNAVAILABLE"}), 503
    except requests.exceptions.Timeout:
        return jsonify({"error": "сервис данных не ответил вовремя", "code": "TIMEOUT"}), 504

    return jsonify(resp.json()), resp.status_code


@app.post("/login")
def login():
    payload = {
        "name": request.form.get("name", "").strip(),
        "mail": request.form.get("mail", "").strip().lower(),
        "password": request.form.get("password", ""),
    }

    try:
        resp = requests.post(
            f"{RUST_SERVICE_URL}/auth/login",
            json=payload,
            timeout=RUST_TIMEOUT,
        )
    except requests.exceptions.ConnectionError:
        return jsonify({"error": "сервис данных недоступен", "code": "SERVICE_UNAVAILABLE"}), 503
    except requests.exceptions.Timeout:
        return jsonify({"error": "сервис данных не ответил вовремя", "code": "TIMEOUT"}), 504

    return jsonify(resp.json()), resp.status_code


if __name__ == "__main__":
    # 0.0.0.0 — слушать все интерфейсы (нужно для контейнера); порт 8000 совпадает с docker-compose.
    app.run(host="0.0.0.0", port=8000)
