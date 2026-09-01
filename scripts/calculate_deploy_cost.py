#!/usr/bin/env python3
"""
Solana Program Deploy Cost Calculator

Calculates the cost to deploy Solana programs including:
- Program deployment fees (write locked accounts)
- Rent exemption for PDA accounts
"""

import subprocess
import sys
import os
from pathlib import Path
from dataclasses import dataclass
from typing import Optional


@dataclass
class ProgramInfo:
    name: str
    path: str
    size: Optional[int] = None
    deploy_fee: float = 0.0
    chunks: int = 0


@dataclass
class PdaInfo:
    name: str
    size: int
    rent_sol: float = 0.0


def run_command(cmd: list[str]) -> str:
    """Run a shell command and return stdout."""
    result = subprocess.run(cmd, capture_output=True, text=True)
    return result.stdout.strip()


def get_file_size(path: str) -> int:
    """Get file size in bytes."""
    return os.path.getsize(path)


def calculate_rent(size: int) -> float:
    """Calculate rent exemption using solana CLI."""
    try:
        output = run_command(["solana", "rent", str(size)])
        for line in output.split("\n"):
            if "Rent-exempt minimum" in line:
                sol_value = line.split(":")[1].strip().replace(" SOL", "")
                return float(sol_value)
    except (subprocess.CalledProcessError, FileNotFoundError, ValueError):
        pass
    
    # Fallback: calculate manually (rent per byte-year = 3480 lamports)
    lamports_per_byte_year = 3480
    ms_per_year = 365 * 24 * 3600 * 1000
    # Minimum 2 years
    rent_per_byte = (lamports_per_byte_year * 2) / 1_000_000_000
    return size * rent_per_byte


def calculate_deploy_fee(size: int, chunk_size: int = 1024, fee_per_chunk: float = 0.000005) -> tuple[int, float]:
    """Calculate deployment fee based on program size."""
    chunks = (size // chunk_size) + 1
    fee = chunks * fee_per_chunk
    return chunks, fee


def format_sol(sol: float) -> str:
    """Format SOL value with appropriate precision."""
    if sol < 0.01:
        return f"{sol:.6f}"
    elif sol < 1:
        return f"{sol:.4f}"
    else:
        return f"{sol:.2f}"


def format_bytes(size: int) -> str:
    """Format bytes to KB."""
    kb = size / 1024
    return f"{size:,} bytes ({kb:.2f} KB)"


def main():
    project_root = Path(__file__).parent.parent
    deploy_dir = project_root / "target" / "deploy"
    
    print("=" * 60)
    print("  Solana Program Deploy Cost Calculator")
    print("=" * 60)
    print()
    
    # Define programs
    programs = [
        ProgramInfo("splitter", str(deploy_dir / "splitter.so")),
        ProgramInfo("splitter_light", str(deploy_dir / "splitter_light.so")),
    ]
    
    # Get program sizes
    print("Program sizes:")
    print("-" * 60)
    for prog in programs:
        if os.path.exists(prog.path):
            prog.size = get_file_size(prog.path)
            prog.chunks, prog.deploy_fee = calculate_deploy_fee(prog.size)
            print(f"  {prog.name:20s} {format_bytes(prog.size)}")
            print(f"                       {prog.chunks} chunks @ 0.000005 SOL = {format_sol(prog.deploy_fee)} SOL")
        else:
            print(f"  {prog.name:20s} NOT FOUND (run: cargo build-sbf)")
    print()
    
    # Define PDA accounts for splitter
    pdas = [
        PdaInfo("Config", 234),  # 8 + 32 + 64 + 32 + 32 + 32 + 32 + 1 + 1
        PdaInfo("TokenList (max 20)", 685),  # 8 + 32 + 4 + (20 * 32) + 1
        PdaInfo("ProfilesIndex (max 10)", 793),  # 8 + 4 + (10 * 77) + 1 + 1
    ]
    
    # Calculate rent for each PDA
    print("PDA Rent Exemption:")
    print("-" * 60)
    total_rent = 0.0
    for pda in pdas:
        pda.rent_sol = calculate_rent(pda.size)
        total_rent += pda.rent_sol
        print(f"  {pda.name:25s} {pda.size:4d} bytes  =  {format_sol(pda.rent_sol)} SOL")
    print("-" * 60)
    print(f"  {'TOTAL PDA RENT':25s} {format_sol(total_rent)} SOL")
    print()
    
    # Calculate totals
    print("=" * 60)
    print("  TOTAL DEPLOY COST")
    print("=" * 60)
    
    splitter = programs[0]
    if splitter.size:
        splitter_total = splitter.deploy_fee + total_rent
        print(f"  splitter:")
        print(f"    Program deployment:  {format_sol(splitter.deploy_fee)} SOL")
        print(f"    PDA rent:            {format_sol(total_rent)} SOL")
        print(f"    ─────────────────────────────────────")
        print(f"    TOTAL:               {format_sol(splitter_total)} SOL")
        print()
    
    light = programs[1]
    if light.size:
        print(f"  splitter_light:")
        print(f"    Program deployment:  {format_sol(light.deploy_fee)} SOL")
        print(f"    (no PDA initialization)")
        print(f"    ─────────────────────────────────────")
        print(f"    TOTAL:               {format_sol(light.deploy_fee)} SOL")
        print()
    
    print("=" * 60)
    print()
    print("Notes:")
    print("  • PDA rent is refundable when accounts are closed")
    print("  • Program deployment fees are non-refundable")
    print("  • Actual transaction fees may vary based on network conditions")
    print("  • Calculations assume max capacity (20 tokens, 10 routes)")
    print()


if __name__ == "__main__":
    main()
