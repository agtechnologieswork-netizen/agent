#!/usr/bin/env python
"""Test template validation"""

import tempfile
import shutil
from pathlib import Path
from nicegui_agent.generator import AgentGenerator

def test_template_validation():
    """Test template validation in temporary workspace"""
    print("Testing template validation...")
    
    ag = AgentGenerator()
    ag.load_template()
    
    # Create temporary workspace
    with tempfile.TemporaryDirectory() as tmpdir:
        workspace = Path(tmpdir) / "test_workspace"
        workspace.mkdir(parents=True)
        
        # Copy template files to workspace
        template_dir = Path("nicegui_agent/template")
        for src_file in template_dir.glob("**/*.py"):
            rel_path = src_file.relative_to(template_dir)
            dest_file = workspace / rel_path
            dest_file.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(src_file, dest_file)
        
        # Copy rules
        rules_dir = workspace / "rules"
        rules_dir.mkdir(exist_ok=True)
        for rule_file in (template_dir / "rules").glob("*.yml"):
            shutil.copy2(rule_file, rules_dir / rule_file.name)
        
        # Run validators
        print(f"\nRunning validators in workspace: {workspace}")
        ag.test_validators({
            "name": "test_template",
            "directory": str(workspace),
            "validators": ag.template_config["templates"][0].get("validators", [])
        })

if __name__ == "__main__":
    test_template_validation()