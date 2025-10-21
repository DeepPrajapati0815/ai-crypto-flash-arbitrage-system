#!/bin/bash
# ✅ ISSUE #1 FIX: Production Build Validation Script (REAL LOGIC - Linux/Mac)

set -e

ENVIRONMENT="${ENVIRONMENT:-development}"
DATA_SOURCE="${DATA_SOURCE:-synthetic}"
MODEL_PATH="${MODEL_PATH:-ml_training/models/arbitrage_model.onnx}"

echo ""
echo "╔════════════════════════════════════════════════════════════╗"
echo "║  ✅ PRODUCTION BUILD VALIDATION (ISSUE #1 FIX)            ║"
echo "╚════════════════════════════════════════════════════════════╝"
echo ""

VALIDATION_ERRORS=()
VALIDATION_WARNINGS=()

# ✅ REAL PRODUCTION LOGIC: Check 1 - Synthetic data forbidden in production
if [ "$ENVIRONMENT" = "production" ] && [ "$DATA_SOURCE" = "synthetic" ]; then
    VALIDATION_ERRORS+=("❌ CRITICAL: Cannot use synthetic data in production environment!")
    VALIDATION_ERRORS+=("   Required: DATA_SOURCE=csv OR DATA_SOURCE=database")
    VALIDATION_ERRORS+=("   Current: DATA_SOURCE=$DATA_SOURCE")
fi

# Check 2: Verify ONNX model exists
if [ ! -f "$MODEL_PATH" ]; then
    VALIDATION_ERRORS+=("❌ CRITICAL: ONNX model not found at: $MODEL_PATH")
    VALIDATION_ERRORS+=("   Run training first: python ml_training/scripts/train_xgboost_model.py")
fi

# Check 3: Verify model metadata (trained on real data)
MODEL_METADATA="${MODEL_PATH%.onnx}_metadata.json"
if [ -f "$MODEL_METADATA" ]; then
    DATA_SOURCE_FROM_MODEL=$(jq -r '.data_source // "unknown"' "$MODEL_METADATA")
    N_SAMPLES=$(jq -r '.n_samples // 0' "$MODEL_METADATA")
    ACCURACY=$(jq -r '.accuracy // 0' "$MODEL_METADATA")
    
    echo "📊 Model Metadata:"
    echo "   Data Source: $DATA_SOURCE_FROM_MODEL"
    echo "   Training Samples: $N_SAMPLES"
    echo "   Accuracy: ${ACCURACY}%"
    echo ""
    
    if [ "$ENVIRONMENT" = "production" ] && [ "$DATA_SOURCE_FROM_MODEL" = "synthetic" ]; then
        VALIDATION_ERRORS+=("❌ CRITICAL: Model was trained on SYNTHETIC data!")
        VALIDATION_ERRORS+=("   Retrain model on real data before production deployment")
    fi
    
    if [ "$ENVIRONMENT" = "production" ] && [ "$N_SAMPLES" -lt 10000 ]; then
        VALIDATION_WARNINGS+=("⚠️  WARNING: Model trained on only $N_SAMPLES samples")
        VALIDATION_WARNINGS+=("   Recommended minimum: 10,000 samples for production")
    fi
    
    if [ "$ENVIRONMENT" = "production" ] && [ "$(echo "$ACCURACY < 75.0" | bc)" -eq 1 ]; then
        VALIDATION_WARNINGS+=("⚠️  WARNING: Model accuracy is only ${ACCURACY}%")
        VALIDATION_WARNINGS+=("   Recommended minimum: 75% accuracy for production")
    fi
else
    VALIDATION_WARNINGS+=("⚠️  WARNING: Model metadata not found at: $MODEL_METADATA")
fi

# Check 4: Historical data CSV
if [ "$DATA_SOURCE" = "csv" ]; then
    CSV_PATH="ml_training/data/historical_ticks.csv"
    if [ ! -f "$CSV_PATH" ]; then
        VALIDATION_ERRORS+=("❌ CRITICAL: Historical data CSV not found: $CSV_PATH")
    else
        LINE_COUNT=$(wc -l < "$CSV_PATH")
        echo "📊 Historical Data CSV: $LINE_COUNT rows"
        if [ "$ENVIRONMENT" = "production" ] && [ "$LINE_COUNT" -lt 10000 ]; then
            VALIDATION_WARNINGS+=("⚠️  WARNING: Only $LINE_COUNT rows in historical data")
        fi
    fi
fi

# Display results
echo ""
echo "═══════════════════════════════════════════════════════════"
echo "VALIDATION RESULTS"
echo "═══════════════════════════════════════════════════════════"
echo ""

if [ ${#VALIDATION_ERRORS[@]} -eq 0 ] && [ ${#VALIDATION_WARNINGS[@]} -eq 0 ]; then
    echo "✅ ALL CHECKS PASSED!"
    echo ""
    echo "Environment: $ENVIRONMENT"
    echo "Data Source: $DATA_SOURCE"
    echo ""
    echo "🚀 Ready for deployment!"
    exit 0
fi

if [ ${#VALIDATION_WARNINGS[@]} -gt 0 ]; then
    echo "⚠️  WARNINGS (${#VALIDATION_WARNINGS[@]}):"
    echo ""
    for warning in "${VALIDATION_WARNINGS[@]}"; do
        echo "  $warning"
    done
    echo ""
fi

if [ ${#VALIDATION_ERRORS[@]} -gt 0 ]; then
    echo "❌ ERRORS (${#VALIDATION_ERRORS[@]}):"
    echo ""
    for error in "${VALIDATION_ERRORS[@]}"; do
        echo "  $error"
    done
    echo ""
    echo "╔════════════════════════════════════════════════════════════╗"
    echo "║  ❌ VALIDATION FAILED - DEPLOYMENT BLOCKED                ║"
    echo "╚════════════════════════════════════════════════════════════╝"
    echo ""
    exit 1
fi

echo "✅ Validation complete with warnings"
exit 0

