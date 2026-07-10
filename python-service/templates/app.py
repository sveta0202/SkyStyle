from flask import Flask, render_template, request, jsonify

app = Flask(__name__)
@app.get("/")
def index():
    return render_template("register.html")

@app.post("/collect-registration")
def collect_registration():
    data = request.get_json(silent=True) or {}
    registration = data.get("registration", {})
    email = registration.get("email", "").strip().lower()
    password = registration.get("password", "")
    user_name = registration.get("user_name", "").strip()

    normalized = {
        "event": "user_registration",
        "user": {
            "user_name": user_name,
            "email": email,
            "password": password
        },
        "meta": data.get("source", {})
    }

    microservice_r = microservice(normalized)
    return jsonify({
        "ok": True,
        "stage": "accepted",
        "normalized_payload": normalized,
        "microservice_response": microservice_r
    })

def microservice(payload: dict):
    return {
        "service": "auth-profile-service",
        "status": "queued",
        "message": "Регистрация принята и отправлена в микросервис",
        "payload_echo": payload
    }

if __name__ == "__main__":
    app.run(debug=True, port=5000)