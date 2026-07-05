"""配置加载分层与优先级单测(config.py TOML 文件层,ADR 0018)。

通过 monkeypatch 覆写模块级配置文件路径,验证:
默认值 · 文件覆盖 · 项目级优先于用户级 · 环境变量优先于文件。
"""

from __future__ import annotations

import textwrap
from pathlib import Path

import pytest

from kairos.foundation import config as config_module
from kairos.foundation.config import KairosSettings


def _write(path: Path, content: str) -> None:
    path.write_text(textwrap.dedent(content), encoding="utf-8")


@pytest.fixture
def isolated_config(
    monkeypatch: pytest.MonkeyPatch, tmp_path: Path
) -> tuple[pytest.MonkeyPatch, Path]:
    """默认把两处配置文件指向不存在的路径,避免读到真实机器上的配置。"""
    monkeypatch.setattr(config_module, "PROJECT_CONFIG_FILE", tmp_path / "nope.toml")
    monkeypatch.setattr(config_module, "USER_CONFIG_FILE", tmp_path / "nouser.toml")
    return monkeypatch, tmp_path


class TestConfigFileLayer:
    def test_defaults_when_no_file(self, isolated_config: tuple[pytest.MonkeyPatch, Path]) -> None:
        settings = KairosSettings()
        assert settings.log_level == "INFO"
        assert settings.embedding.model == "BAAI/bge-m3"

    def test_toml_overrides_defaults(
        self, isolated_config: tuple[pytest.MonkeyPatch, Path]
    ) -> None:
        monkeypatch, tmp_path = isolated_config
        proj = tmp_path / "config.toml"
        _write(
            proj,
            """
            log_level = "DEBUG"

            [embedding]
            model = "custom-model"
            base_url = "http://localhost:8000/v1"
            """,
        )
        monkeypatch.setattr(config_module, "PROJECT_CONFIG_FILE", proj)

        settings = KairosSettings()
        assert settings.log_level == "DEBUG"
        assert settings.embedding.model == "custom-model"
        assert settings.embedding.base_url == "http://localhost:8000/v1"
        # 文件未覆盖的字段回落代码默认值
        assert settings.embedding.dim == 1024

    def test_project_file_overrides_user_file(
        self, isolated_config: tuple[pytest.MonkeyPatch, Path]
    ) -> None:
        monkeypatch, tmp_path = isolated_config
        user = tmp_path / "user.toml"
        _write(user, '\nlog_level = "WARNING"\n\n[embedding]\nmodel = "user-model"\n')
        proj = tmp_path / "config.toml"
        _write(proj, '\n[embedding]\nmodel = "proj-model"\n')
        monkeypatch.setattr(config_module, "PROJECT_CONFIG_FILE", proj)
        monkeypatch.setattr(config_module, "USER_CONFIG_FILE", user)

        settings = KairosSettings()
        assert settings.embedding.model == "proj-model"  # 项目级优先
        assert settings.log_level == "WARNING"  # 项目级未设,回落用户级

    def test_env_var_overrides_file(self, isolated_config: tuple[pytest.MonkeyPatch, Path]) -> None:
        monkeypatch, tmp_path = isolated_config
        proj = tmp_path / "config.toml"
        _write(proj, '\n[embedding]\nmodel = "file-model"\n')
        monkeypatch.setattr(config_module, "PROJECT_CONFIG_FILE", proj)
        monkeypatch.setenv("KAIROS_EMBEDDING__MODEL", "env-model")

        settings = KairosSettings()
        assert settings.embedding.model == "env-model"  # 环境变量最高优先级
