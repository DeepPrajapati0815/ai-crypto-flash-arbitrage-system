# =============================================================================
# ML Training Service - Dockerfile
# =============================================================================
# This container trains ML models and exports them to ONNX format

FROM python:3.11-slim

# Install system dependencies
RUN apt-get update && apt-get install -y \
    build-essential \
    libpq-dev \
    curl \
    && rm -rf /var/lib/apt/lists/*

# Set working directory
WORKDIR /ml

# Copy requirements
COPY requirements-fast.txt .

# Install Python dependencies with increased timeout and retries
RUN pip install --default-timeout=1000 --retries 5 -r requirements-fast.txt

# Copy training scripts and data
COPY scripts/ ./scripts/
COPY data/ ./data/

# Create models output directory
RUN mkdir -p models

# Set environment variables
ENV PYTHONUNBUFFERED=1
ENV PYTHONPATH=/ml

# Default command: train XGBoost model
CMD ["python", "scripts/train_xgboost_model.py"]

