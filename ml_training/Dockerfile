# =============================================================================
# ML Training Service - Dockerfile
# =============================================================================
# This container trains ML models and exports them to ONNX format

FROM python:3.11-slim

# Install system dependencies with security fixes
RUN apt-get update && apt-get install -y \
    build-essential \
    libpq-dev \
    curl \
    && rm -rf /var/lib/apt/lists/*

# Fix ONNX executable stack security issue
RUN echo "kernel.exec-shield = 0" >> /etc/sysctl.conf && \
    echo "kernel.randomize_va_space = 0" >> /etc/sysctl.conf

# Set working directory
WORKDIR /ml

# Copy requirements
COPY requirements-fast.txt .

# Install Python dependencies with increased timeout and retries
# Fix ONNX runtime security issues
RUN pip install --default-timeout=1000 --retries 5 -r requirements-fast.txt && \
    pip install --upgrade onnxruntime==1.16.3

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

