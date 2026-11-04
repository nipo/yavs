-- A signal whose subtype has more than 2**64 - 1 elements is E1406 at elaboration, not a
-- wrapped size (#1092). No aggregate is involved; the subtype alone is too large.
entity array_too_large_signal is
end entity;

architecture a of array_too_large_signal is
  type big is range 0 to 4294967295;
  type m is array (big, big) of bit;
  signal s : m;
begin
  process
  begin
    report "unreachable";
    wait;
  end process;
end architecture;
