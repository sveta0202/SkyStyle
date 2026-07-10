FROM python:3.11-slim

WORKDIR /app

# Зависимости ставим до копирования кода — кэшируется при неизменном requirements.txt
COPY requirements.txt ./
RUN pip install --no-cache-dir -r requirements.txt

# Копируем приложение, шаблоны и статику (CSS/картинки)
COPY app.py ./
COPY templates ./templates
COPY static ./static

EXPOSE 8000

CMD ["python", "app.py"]
