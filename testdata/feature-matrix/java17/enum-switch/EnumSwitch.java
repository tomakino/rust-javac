public class EnumSwitch {
    enum Mode {
        ON, OFF, IDLE
    }

    static int weight(Mode mode) {
        switch (mode) {
            case ON:
                return 1;
            case OFF:
                return 2;
            default:
                return 3;
        }
    }

    public static void main(String[] args) {
        System.out.println(weight(Mode.ON) + weight(Mode.IDLE));
    }
}
