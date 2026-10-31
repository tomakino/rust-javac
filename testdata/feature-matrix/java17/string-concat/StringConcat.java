public class StringConcat {
    public static void main(String[] args) {
        int number = 7;
        Object nothing = null;
        String text = "a" + number + "b" + (number + 1) + nothing + 'c' + 2.5;
        System.out.println(text);
    }
}
