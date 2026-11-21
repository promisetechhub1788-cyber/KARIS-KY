from backend import backup_escrow_state as backup
from stellar_sdk.contract import ContractClient


class FakeServer:
    server_url = "https://rpc.example.test"


def test_build_snapshot_with_successful_rpc_responses(monkeypatch):
    calls = []

    def invoke(self, function_name, args, source=None):
        calls.append(function_name)
        if function_name == "get_escrow":
            return {"status": 0, "funded_amount": 125}
        if function_name == "get_unique_funder_count":
            return 2
        return {"entrypoint": function_name}

    monkeypatch.setattr(ContractClient, "invoke", invoke)

    snapshot = backup.build_snapshot(
        FakeServer(), "C" + "A" * 55, "test network", []
    )

    assert snapshot["escrow"] == {"status": 0, "funded_amount": 125}
    assert snapshot["unique_funder_count"] == 2
    assert snapshot["funding_token"] == {"entrypoint": "get_funding_token"}
    assert len(calls) == 16


def test_build_snapshot_records_rpc_errors(monkeypatch):
    def invoke(self, function_name, args, source=None):
        if function_name == "get_escrow":
            raise RuntimeError("RPC unavailable")
        return function_name

    monkeypatch.setattr(ContractClient, "invoke", invoke)

    snapshot = backup.build_snapshot(
        FakeServer(), "C" + "A" * 55, "test network", []
    )

    assert snapshot["escrow"] == {"__error": "RPC unavailable"}
    assert snapshot["schema_version"] == "get_version"


def test_build_snapshot_records_malformed_rpc_responses(monkeypatch):
    def invoke(self, function_name, args, source=None):
        if function_name == "get_escrow":
            raise ValueError("malformed simulation response")
        return function_name

    monkeypatch.setattr(ContractClient, "invoke", invoke)

    snapshot = backup.build_snapshot(
        FakeServer(), "C" + "A" * 55, "test network", []
    )

    assert snapshot["escrow"] == {"__error": "malformed simulation response"}