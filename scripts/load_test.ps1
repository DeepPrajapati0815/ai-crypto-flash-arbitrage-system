# Load Testing Script
# Simulates high-frequency trading load to validate system performance

param(
    [int]$DurationMinutes = 5,
    [int]$RequestsPerSecond = 100,
    [string]$TargetUrl = "http://localhost:8080"
)

Write-Host "=====================================" -ForegroundColor Cyan
Write-Host "      Load Testing Suite             " -ForegroundColor Cyan
Write-Host "=====================================" -ForegroundColor Cyan
Write-Host ""
Write-Host "Configuration:" -ForegroundColor Blue
Write-Host "  Duration: $DurationMinutes minutes" -ForegroundColor Gray
Write-Host "  Target RPS: $RequestsPerSecond" -ForegroundColor Gray
Write-Host "  Target URL: $TargetUrl" -ForegroundColor Gray
Write-Host ""

# Check if system is running
Write-Host "🔍 Checking if system is running..." -ForegroundColor Blue

try {
    $response = Invoke-WebRequest -Uri "$TargetUrl/health" -TimeoutSec 5 -UseBasicParsing
    if ($response.StatusCode -eq 200) {
        Write-Host "✅ System is running" -ForegroundColor Green
    } else {
        Write-Host "❌ System returned status code $($response.StatusCode)" -ForegroundColor Red
        exit 1
    }
} catch {
    Write-Host "❌ System is not responding at $TargetUrl" -ForegroundColor Red
    Write-Host "   Please start the system first with: .\run.ps1" -ForegroundColor Yellow
    exit 1
}

Write-Host ""

# Check if metrics endpoint is available
Write-Host "📊 Checking metrics endpoint..." -ForegroundColor Blue
try {
    $response = Invoke-WebRequest -Uri "$TargetUrl/metrics" -TimeoutSec 5 -UseBasicParsing
    if ($response.StatusCode -eq 200) {
        Write-Host "✅ Metrics endpoint available" -ForegroundColor Green
    }
} catch {
    Write-Host "⚠️  Metrics endpoint not available" -ForegroundColor Yellow
}

Write-Host ""

# Start load test
Write-Host "🚀 Starting load test..." -ForegroundColor Blue
Write-Host ""

$startTime = Get-Date
$endTime = $startTime.AddMinutes($DurationMinutes)
$intervalMs = [int]((1000.0 / $RequestsPerSecond) * 10) # Send 10 requests per interval

$successCount = 0
$errorCount = 0
$totalLatency = 0

$currentSecond = 0
$requestsThisSecond = 0

Write-Host "Time (s) | Requests | Success | Errors | Avg Latency (ms)" -ForegroundColor Cyan
Write-Host "---------|----------|---------|--------|------------------" -ForegroundColor Cyan

while ((Get-Date) -lt $endTime) {
    $batchStart = Get-Date
    
    # Send batch of requests
    $jobs = @()
    for ($i = 0; $i -lt 10; $i++) {
        $jobs += Start-Job -ScriptBlock {
            param($url)
            $sw = [System.Diagnostics.Stopwatch]::StartNew()
            try {
                $response = Invoke-WebRequest -Uri "$url/health" -TimeoutSec 2 -UseBasicParsing
                $sw.Stop()
                return @{
                    Success = ($response.StatusCode -eq 200)
                    Latency = $sw.ElapsedMilliseconds
                }
            } catch {
                $sw.Stop()
                return @{
                    Success = $false
                    Latency = $sw.ElapsedMilliseconds
                }
            }
        } -ArgumentList $TargetUrl
        
        $requestsThisSecond++
    }
    
    # Wait for batch to complete
    $results = $jobs | Wait-Job | Receive-Job
    $jobs | Remove-Job
    
    # Process results
    foreach ($result in $results) {
        if ($result.Success) {
            $successCount++
        } else {
            $errorCount++
        }
        $totalLatency += $result.Latency
    }
    
    # Check if second elapsed
    $elapsedSeconds = [int]((Get-Date) - $startTime).TotalSeconds
    if ($elapsedSeconds -gt $currentSecond) {
        $avgLatency = if (($successCount + $errorCount) -gt 0) {
            [int]($totalLatency / ($successCount + $errorCount))
        } else {
            0
        }
        
        Write-Host ("{0,8} | {1,8} | {2,7} | {3,6} | {4,16}" -f `
            $elapsedSeconds, `
            $requestsThisSecond, `
            $successCount, `
            $errorCount, `
            $avgLatency) -ForegroundColor $(if ($errorCount -eq 0) { "Green" } else { "Yellow" })
        
        $currentSecond = $elapsedSeconds
        $requestsThisSecond = 0
    }
    
    # Sleep to maintain target RPS
    $batchDuration = ((Get-Date) - $batchStart).TotalMilliseconds
    $sleepTime = [Math]::Max(0, $intervalMs - $batchDuration)
    Start-Sleep -Milliseconds $sleepTime
}

Write-Host ""
Write-Host "=====================================" -ForegroundColor Cyan
Write-Host "      Load Test Complete             " -ForegroundColor Cyan
Write-Host "=====================================" -ForegroundColor Cyan
Write-Host ""

$totalRequests = $successCount + $errorCount
$successRate = if ($totalRequests -gt 0) {
    [Math]::Round(($successCount / $totalRequests) * 100, 2)
} else {
    0
}
$avgLatency = if ($totalRequests -gt 0) {
    [Math]::Round($totalLatency / $totalRequests, 2)
} else {
    0
}
$actualRps = [Math]::Round($totalRequests / $DurationMinutes / 60, 2)

Write-Host "Results:" -ForegroundColor Blue
Write-Host "  Total Requests: $totalRequests" -ForegroundColor Gray
Write-Host "  Successful: $successCount" -ForegroundColor Green
Write-Host "  Failed: $errorCount" -ForegroundColor $(if ($errorCount -eq 0) { "Green" } else { "Red" })
Write-Host "  Success Rate: $successRate%" -ForegroundColor $(if ($successRate -gt 95) { "Green" } elseif ($successRate -gt 90) { "Yellow" } else { "Red" })
Write-Host "  Average Latency: ${avgLatency}ms" -ForegroundColor $(if ($avgLatency -lt 100) { "Green" } elseif ($avgLatency -lt 500) { "Yellow" } else { "Red" })
Write-Host "  Actual RPS: $actualRps" -ForegroundColor Gray
Write-Host ""

# Fetch final metrics
Write-Host "📊 Final metrics from system:" -ForegroundColor Blue
try {
    $metrics = Invoke-WebRequest -Uri "$TargetUrl/metrics" -UseBasicParsing
    $metricsText = $metrics.Content
    
    # Extract key metrics
    if ($metricsText -match 'arbitrage_opportunities_detected_total (\d+)') {
        Write-Host "  Opportunities Detected: $($matches[1])" -ForegroundColor Gray
    }
    if ($metricsText -match 'trades_executed_total (\d+)') {
        Write-Host "  Trades Executed: $($matches[1])" -ForegroundColor Gray
    }
    if ($metricsText -match 'trades_failed_total (\d+)') {
        Write-Host "  Trades Failed: $($matches[1])" -ForegroundColor Gray
    }
    if ($metricsText -match 'memory_usage_megabytes (\d+)') {
        Write-Host "  Memory Usage: $($matches[1]) MB" -ForegroundColor Gray
    }
    if ($metricsText -match 'cpu_usage_percent (\d+)') {
        Write-Host "  CPU Usage: $($matches[1])%" -ForegroundColor Gray
    }
} catch {
    Write-Host "  ⚠️  Could not fetch metrics" -ForegroundColor Yellow
}

Write-Host ""

if ($successRate -gt 95 -and $avgLatency -lt 500) {
    Write-Host "🎉 Load test PASSED!" -ForegroundColor Green
    exit 0
} elseif ($successRate -gt 90) {
    Write-Host "⚠️  Load test completed with warnings" -ForegroundColor Yellow
    exit 0
} else {
    Write-Host "❌ Load test FAILED - System performance degraded" -ForegroundColor Red
    exit 1
}

