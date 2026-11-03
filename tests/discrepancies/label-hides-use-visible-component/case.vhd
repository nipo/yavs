-- The label is directly visible, so it hides the component made potentially visible by the use clause
-- (11.1, 12.4), and the instantiated name is the label.
package pkg is
  component fifo is end component;
end package;

entity fifo is end entity;
architecture a of fifo is
begin
  p : process begin report "RES accepted"; wait; end process;
end architecture;

use work.pkg.all;
entity top is end entity;
architecture a of top is
begin
  fifo : fifo;
end architecture;
