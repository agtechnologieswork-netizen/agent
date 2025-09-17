import { Admin } from "react-admin";
import simpleRestProvider from "ra-data-simple-rest";

const dataProvider = simpleRestProvider("/api");

export const App = () => (
  <Admin dataProvider={dataProvider}>
    {/* TODO: Add your resources here */}
    {/* 
    Example usage:
    
    import { Resource, ListGuesser, EditGuesser, ShowGuesser } from "react-admin";
    
    <Resource 
      name="users" 
      list={ListGuesser}
      edit={EditGuesser}
      show={ShowGuesser}
    />
    */}
  </Admin>
);