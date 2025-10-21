# Production Readiness Validation Script
# This script validates that the system is ready for production deployment

param(
    [switch]$Verbose,
    [string]$ConfigFile = "config/production.json",
    [int]$TimeoutSeconds = 300
)

# Set error action preference
$ErrorActionPreference = "Stop"

# Import required modules
Import-Module -Name "PSReadLine" -ErrorAction SilentlyContinue

# Configuration
$Config = @{
    RequiredScore = 95
    CriticalChecks = @(
        "system_initialization",
        "database_connectivity", 
        "redis_connectivity",
        "security_configuration",
        "key_management",
        "authentication",
        "authorization"
    )
    WarningThresholds = @{
        Latency = 100.0
        MemoryUsage = 80.0
        CPUUsage = 80.0
        DiskUsage = 80.0
    }
}

# Load configuration from file if it exists
if (Test-Path $ConfigFile) {
    try {
        $FileConfig = Get-Content $ConfigFile | ConvertFrom-Json
        $Config = $Config + $FileConfig
        Write-Host "Loaded configuration from $ConfigFile" -ForegroundColor Green
    }
    catch {
        Write-Warning "Failed to load configuration from $ConfigFile: $_"
    }
}

# Function to write colored output
function Write-ColorOutput {
    param(
        [string]$Message,
        [string]$Color = "White"
    )
    
    if ($Verbose) {
        Write-Host "[$(Get-Date -Format 'HH:mm:ss')] $Message" -ForegroundColor $Color
    }
}

# Function to check system requirements
function Test-SystemRequirements {
    Write-ColorOutput "Checking system requirements..." "Yellow"
    
    $Results = @{
        Passed = 0
        Failed = 0
        Warnings = 0
        Details = @()
    }
    
    # Check OS version
    $OSVersion = [System.Environment]::OSVersion.Version
    if ($OSVersion.Major -ge 10) {
        $Results.Passed++
        $Results.Details += "OS Version: Windows 10+ ✓"
    } else {
        $Results.Failed++
        $Results.Details += "OS Version: Windows 10+ required ✗"
    }
    
    # Check available memory
    $Memory = Get-WmiObject -Class Win32_ComputerSystem
    $TotalMemoryGB = [math]::Round($Memory.TotalPhysicalMemory / 1GB, 2)
    if ($TotalMemoryGB -ge 16) {
        $Results.Passed++
        $Results.Details += "Memory: ${TotalMemoryGB}GB ✓"
    } else {
        $Results.Warnings++
        $Results.Details += "Memory: ${TotalMemoryGB}GB (16GB+ recommended) ⚠"
    }
    
    # Check CPU cores
    $CPU = Get-WmiObject -Class Win32_Processor
    $CoreCount = $CPU.NumberOfCores * $CPU.NumberOfLogicalProcessors
    if ($CoreCount -ge 8) {
        $Results.Passed++
        $Results.Details += "CPU Cores: $CoreCount ✓"
    } else {
        $Results.Warnings++
        $Results.Details += "CPU Cores: $CoreCount (8+ recommended) ⚠"
    }
    
    # Check disk space
    $Disk = Get-WmiObject -Class Win32_LogicalDisk -Filter "DriveType=3"
    $FreeSpaceGB = [math]::Round($Disk.FreeSpace / 1GB, 2)
    if ($FreeSpaceGB -ge 100) {
        $Results.Passed++
        $Results.Details += "Disk Space: ${FreeSpaceGB}GB free ✓"
    } else {
        $Results.Warnings++
        $Results.Details += "Disk Space: ${FreeSpaceGB}GB free (100GB+ recommended) ⚠"
    }
    
    return $Results
}

# Function to check network connectivity
function Test-NetworkConnectivity {
    Write-ColorOutput "Checking network connectivity..." "Yellow"
    
    $Results = @{
        Passed = 0
        Failed = 0
        Warnings = 0
        Details = @()
    }
    
    # Test internet connectivity
    try {
        $Ping = Test-Connection -ComputerName "8.8.8.8" -Count 1 -Quiet
        if ($Ping) {
            $Results.Passed++
            $Results.Details += "Internet connectivity: OK ✓"
        } else {
            $Results.Failed++
            $Results.Details += "Internet connectivity: Failed ✗"
        }
    }
    catch {
        $Results.Failed++
        $Results.Details += "Internet connectivity: Failed ✗"
    }
    
    # Test DNS resolution
    try {
        $DNS = Resolve-DnsName -Name "google.com" -ErrorAction Stop
        if ($DNS) {
            $Results.Passed++
            $Results.Details += "DNS resolution: OK ✓"
        } else {
            $Results.Failed++
            $Results.Details += "DNS resolution: Failed ✗"
        }
    }
    catch {
        $Results.Failed++
        $Results.Details += "DNS resolution: Failed ✗"
    }
    
    # Test exchange connectivity
    $Exchanges = @("binance.com", "okx.com", "coinbase.com")
    foreach ($Exchange in $Exchanges) {
        try {
            $Response = Invoke-WebRequest -Uri "https://$Exchange" -TimeoutSec 10 -UseBasicParsing
            if ($Response.StatusCode -eq 200) {
                $Results.Passed++
                $Results.Details += "Exchange connectivity ($Exchange): OK ✓"
            } else {
                $Results.Warnings++
                $Results.Details += "Exchange connectivity ($Exchange): Status $($Response.StatusCode) ⚠"
            }
        }
        catch {
            $Results.Warnings++
            $Results.Details += "Exchange connectivity ($Exchange): Failed ⚠"
        }
    }
    
    return $Results
}

# Function to check database connectivity
function Test-DatabaseConnectivity {
    Write-ColorOutput "Checking database connectivity..." "Yellow"
    
    $Results = @{
        Passed = 0
        Failed = 0
        Warnings = 0
        Details = @()
    }
    
    # Check if PostgreSQL is running
    try {
        $PostgreSQL = Get-Service -Name "postgresql*" -ErrorAction SilentlyContinue
        if ($PostgreSQL -and $PostgreSQL.Status -eq "Running") {
            $Results.Passed++
            $Results.Details += "PostgreSQL service: Running ✓"
        } else {
            $Results.Failed++
            $Results.Details += "PostgreSQL service: Not running ✗"
        }
    }
    catch {
        $Results.Failed++
        $Results.Details += "PostgreSQL service: Not found ✗"
    }
    
    # Check if Redis is running
    try {
        $Redis = Get-Service -Name "redis*" -ErrorAction SilentlyContinue
        if ($Redis -and $Redis.Status -eq "Running") {
            $Results.Passed++
            $Results.Details += "Redis service: Running ✓"
        } else {
            $Results.Failed++
            $Results.Details += "Redis service: Not running ✗"
        }
    }
    catch {
        $Results.Failed++
        $Results.Details += "Redis service: Not found ✗"
    }
    
    return $Results
}

# Function to check security configuration
function Test-SecurityConfiguration {
    Write-ColorOutput "Checking security configuration..." "Yellow"
    
    $Results = @{
        Passed = 0
        Failed = 0
        Warnings = 0
        Details = @()
    }
    
    # Check if environment variables are set
    $RequiredEnvVars = @("DATABASE_URL", "REDIS_URL", "PRIVATE_KEY")
    foreach ($EnvVar in $RequiredEnvVars) {
        if ([System.Environment]::GetEnvironmentVariable($EnvVar)) {
            $Results.Passed++
            $Results.Details += "Environment variable ($EnvVar): Set ✓"
        } else {
            $Results.Failed++
            $Results.Details += "Environment variable ($EnvVar): Not set ✗"
        }
    }
    
    # Check if SSL certificates exist
    if (Test-Path "cert.pem" -and Test-Path "key.pem") {
        $Results.Passed++
        $Results.Details += "SSL certificates: Found ✓"
    } else {
        $Results.Warnings++
        $Results.Details += "SSL certificates: Not found ⚠"
    }
    
    # Check if firewall is configured
    try {
        $Firewall = Get-NetFirewallProfile -Profile Domain,Public,Private
        $EnabledProfiles = $Firewall | Where-Object { $_.Enabled -eq $true }
        if ($EnabledProfiles) {
            $Results.Passed++
            $Results.Details += "Firewall: Enabled ✓"
        } else {
            $Results.Warnings++
            $Results.Details += "Firewall: Not enabled ⚠"
        }
    }
    catch {
        $Results.Warnings++
        $Results.Details += "Firewall: Cannot check ⚠"
    }
    
    return $Results
}

# Function to check application build
function Test-ApplicationBuild {
    Write-ColorOutput "Checking application build..." "Yellow"
    
    $Results = @{
        Passed = 0
        Failed = 0
        Warnings = 0
        Details = @()
    }
    
    # Check if Rust is installed
    try {
        $RustVersion = rustc --version 2>$null
        if ($RustVersion) {
            $Results.Passed++
            $Results.Details += "Rust compiler: $RustVersion ✓"
        } else {
            $Results.Failed++
            $Results.Details += "Rust compiler: Not found ✗"
        }
    }
    catch {
        $Results.Failed++
        $Results.Details += "Rust compiler: Not found ✗"
    }
    
    # Check if Cargo is available
    try {
        $CargoVersion = cargo --version 2>$null
        if ($CargoVersion) {
            $Results.Passed++
            $Results.Details += "Cargo: $CargoVersion ✓"
        } else {
            $Results.Failed++
            $Results.Details += "Cargo: Not found ✗"
        }
    }
    catch {
        $Results.Failed++
        $Results.Details += "Cargo: Not found ✗"
        return $Results
    }
    
    # Check if project builds
    try {
        Write-ColorOutput "Building project..." "Cyan"
        $BuildOutput = cargo build --release 2>&1
        if ($LASTEXITCODE -eq 0) {
            $Results.Passed++
            $Results.Details += "Project build: Successful ✓"
        } else {
            $Results.Failed++
            $Results.Details += "Project build: Failed ✗"
            $Results.Details += "Build output: $BuildOutput"
        }
    }
    catch {
        $Results.Failed++
        $Results.Details += "Project build: Failed ✗"
    }
    
    # Check if tests pass
    try {
        Write-ColorOutput "Running tests..." "Cyan"
        $TestOutput = cargo test --release 2>&1
        if ($LASTEXITCODE -eq 0) {
            $Results.Passed++
            $Results.Details += "Tests: All passed ✓"
        } else {
            $Results.Warnings++
            $Results.Details += "Tests: Some failed ⚠"
            $Results.Details += "Test output: $TestOutput"
        }
    }
    catch {
        $Results.Warnings++
        $Results.Details += "Tests: Failed ⚠"
    }
    
    return $Results
}

# Function to check monitoring setup
function Test-MonitoringSetup {
    Write-ColorOutput "Checking monitoring setup..." "Yellow"
    
    $Results = @{
        Passed = 0
        Failed = 0
        Warnings = 0
        Details = @()
    }
    
    # Check if Prometheus is running
    try {
        $Prometheus = Get-Service -Name "prometheus*" -ErrorAction SilentlyContinue
        if ($Prometheus -and $Prometheus.Status -eq "Running") {
            $Results.Passed++
            $Results.Details += "Prometheus: Running ✓"
        } else {
            $Results.Warnings++
            $Results.Details += "Prometheus: Not running ⚠"
        }
    }
    catch {
        $Results.Warnings++
        $Results.Details += "Prometheus: Not found ⚠"
    }
    
    # Check if Grafana is running
    try {
        $Grafana = Get-Service -Name "grafana*" -ErrorAction SilentlyContinue
        if ($Grafana -and $Grafana.Status -eq "Running") {
            $Results.Passed++
            $Results.Details += "Grafana: Running ✓"
        } else {
            $Results.Warnings++
            $Results.Details += "Grafana: Not running ⚠"
        }
    }
    catch {
        $Results.Warnings++
        $Results.Details += "Grafana: Not found ⚠"
    }
    
    # Check if health endpoints are accessible
    try {
        $HealthResponse = Invoke-WebRequest -Uri "http://localhost:8080/health" -TimeoutSec 5 -UseBasicParsing
        if ($HealthResponse.StatusCode -eq 200) {
            $Results.Passed++
            $Results.Details += "Health endpoint: Accessible ✓"
        } else {
            $Results.Warnings++
            $Results.Details += "Health endpoint: Status $($HealthResponse.StatusCode) ⚠"
        }
    }
    catch {
        $Results.Warnings++
        $Results.Details += "Health endpoint: Not accessible ⚠"
    }
    
    return $Results
}

# Function to calculate readiness score
function Get-ReadinessScore {
    param(
        [hashtable]$Results
    )
    
    $TotalChecks = $Results.Passed + $Results.Failed + $Results.Warnings
    if ($TotalChecks -eq 0) {
        return 0
    }
    
    $Score = [math]::Round(($Results.Passed / $TotalChecks) * 100, 1)
    return $Score
}

# Function to generate report
function New-ReadinessReport {
    param(
        [hashtable]$SystemResults,
        [hashtable]$NetworkResults,
        [hashtable]$DatabaseResults,
        [hashtable]$SecurityResults,
        [hashtable]$BuildResults,
        [hashtable]$MonitoringResults
    )
    
    $Report = @{
        Timestamp = Get-Date -Format "yyyy-MM-dd HH:mm:ss"
        OverallScore = 0
        Status = "Not Ready"
        Summary = @{
            TotalChecks = 0
            Passed = 0
            Failed = 0
            Warnings = 0
        }
        Details = @()
        CriticalIssues = @()
        Recommendations = @()
    }
    
    # Aggregate results
    $AllResults = @($SystemResults, $NetworkResults, $DatabaseResults, $SecurityResults, $BuildResults, $MonitoringResults)
    
    foreach ($Result in $AllResults) {
        $Report.Summary.TotalChecks += $Result.Passed + $Result.Failed + $Result.Warnings
        $Report.Summary.Passed += $Result.Passed
        $Report.Summary.Failed += $Result.Failed
        $Report.Summary.Warnings += $Result.Warnings
        $Report.Details += $Result.Details
    }
    
    # Calculate overall score
    $Report.OverallScore = Get-ReadinessScore $Report.Summary
    
    # Determine status
    if ($Report.OverallScore -ge $Config.RequiredScore -and $Report.Summary.Failed -eq 0) {
        $Report.Status = "Ready"
    } elseif ($Report.Summary.Failed -eq 0 -and $Report.Summary.Warnings -le 3) {
        $Report.Status = "Needs Review"
    } else {
        $Report.Status = "Not Ready"
    }
    
    # Identify critical issues
    foreach ($Detail in $Report.Details) {
        if ($Detail -like "*✗*") {
            $Report.CriticalIssues += $Detail
        }
    }
    
    # Generate recommendations
    if ($Report.Summary.Failed -gt 0) {
        $Report.Recommendations += "Fix all failed checks before deployment"
    }
    if ($Report.Summary.Warnings -gt 0) {
        $Report.Recommendations += "Address warnings for optimal performance"
    }
    if ($Report.OverallScore -lt $Config.RequiredScore) {
        $Report.Recommendations += "Improve overall readiness score to $($Config.RequiredScore)% or higher"
    }
    
    return $Report
}

# Main execution
try {
    Write-Host "AI Crypto Flash Arbitrage System - Production Readiness Validation" -ForegroundColor Green
    Write-Host "=====================================================================" -ForegroundColor Green
    Write-Host ""
    
    # Run all checks
    Write-ColorOutput "Starting production readiness validation..." "Green"
    
    $SystemResults = Test-SystemRequirements
    $NetworkResults = Test-NetworkConnectivity
    $DatabaseResults = Test-DatabaseConnectivity
    $SecurityResults = Test-SecurityConfiguration
    $BuildResults = Test-ApplicationBuild
    $MonitoringResults = Test-MonitoringSetup
    
    # Generate report
    $Report = New-ReadinessReport $SystemResults $NetworkResults $DatabaseResults $SecurityResults $BuildResults $MonitoringResults
    
    # Display results
    Write-Host ""
    Write-Host "Production Readiness Report" -ForegroundColor Yellow
    Write-Host "===========================" -ForegroundColor Yellow
    Write-Host "Timestamp: $($Report.Timestamp)"
    Write-Host "Overall Score: $($Report.OverallScore)%"
    Write-Host "Status: $($Report.Status)"
    Write-Host ""
    
    Write-Host "Summary:" -ForegroundColor Cyan
    Write-Host "  Total Checks: $($Report.Summary.TotalChecks)"
    Write-Host "  Passed: $($Report.Summary.Passed)" -ForegroundColor Green
    Write-Host "  Failed: $($Report.Summary.Failed)" -ForegroundColor Red
    Write-Host "  Warnings: $($Report.Summary.Warnings)" -ForegroundColor Yellow
    Write-Host ""
    
    if ($Report.CriticalIssues.Count -gt 0) {
        Write-Host "Critical Issues:" -ForegroundColor Red
        foreach ($Issue in $Report.CriticalIssues) {
            Write-Host "  - $Issue" -ForegroundColor Red
        }
        Write-Host ""
    }
    
    if ($Report.Recommendations.Count -gt 0) {
        Write-Host "Recommendations:" -ForegroundColor Yellow
        foreach ($Rec in $Report.Recommendations) {
            Write-Host "  - $Rec" -ForegroundColor Yellow
        }
        Write-Host ""
    }
    
    # Display detailed results
    if ($Verbose) {
        Write-Host "Detailed Results:" -ForegroundColor Cyan
        foreach ($Detail in $Report.Details) {
            if ($Detail -like "*✓*") {
                Write-Host "  $Detail" -ForegroundColor Green
            } elseif ($Detail -like "*⚠*") {
                Write-Host "  $Detail" -ForegroundColor Yellow
            } elseif ($Detail -like "*✗*") {
                Write-Host "  $Detail" -ForegroundColor Red
            } else {
                Write-Host "  $Detail"
            }
        }
        Write-Host ""
    }
    
    # Exit with appropriate code
    if ($Report.Status -eq "Ready") {
        Write-Host "System is ready for production deployment!" -ForegroundColor Green
        exit 0
    } elseif ($Report.Status -eq "Needs Review") {
        Write-Host "System needs manual review before deployment." -ForegroundColor Yellow
        exit 1
    } else {
        Write-Host "System is not ready for production deployment." -ForegroundColor Red
        exit 2
    }
}
catch {
    Write-Error "Validation failed: $_"
    exit 3
}
