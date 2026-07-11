#FROM python:3.11-slim
#
#WORKDIR /app
#
#COPY requirements.txt ./
#RUN pip install --no-cache-dir -r requirements.txt
#COPY app.py ./
#COPY templates ./templates
#COPY static ./static
#
#EXPOSE 8000
#
#CMD ["python", "app.py"]

FROM python:3.11-slim

WORKDIR /app

ENV PYTHONDONTWRITEBYTECODE=1
ENV PYTHONUNBUFFERED=1

COPY python-service/requirements.txt ./requirements.txt
RUN pip install --no-cache-dir -r requirements.txt

COPY python-service/ .

CMD ["sh", "-c", "gunicorn app:app --bind 0.0.0.0:$PORT --workers 2 --threads 4 --timeout 120"]