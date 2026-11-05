-- A process may come first, but a target that never exists stays an error.
entity external_alias_process_missing is
end;
architecture t of external_alias_process_missing is
begin
  process
    alias a is << signal .external_alias_process_missing.nope : bit >>;
  begin
    wait;
  end process;
end;
