from flask import Flask, render_template, request, jsonify

app = Flask(__name__)

@app.get("/")
def index():
    return render_template("index.html")

@app.get("/auth")
def auth():
    return render_template("login.html")

@app.get("/landing")
def landing():
    return render_template("index.html")

@app.get("/reg")
def reg():
    return ""

@app.post("/collect-registration")
def collect_registration():
    data = request.get_json(silent=True) or {}
    registration = data.get("registration", {})

    return jsonify({
        "ok": True,
        "user": {
            "user_name": registration.get("user_name", "").strip(),
            "email": registration.get("email", "").strip().lower(),
            "password": registration.get("password", "")
        }
    })

if __name__ == "__main__":
    app.run(debug=True, port=5000)