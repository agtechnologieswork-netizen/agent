import {
  Admin,
  Resource,
  ListGuesser,
  EditGuesser,
  ShowGuesser,
  TextInput,
} from "react-admin";
import { Layout } from "./Layout";
import simpleRestProvider from "ra-data-simple-rest";
import CreateGuesser from "./components/create-guesser";

const dataProvider = simpleRestProvider("/api");

// TO GENERATE: for each resource in the dataProvider, create a Resource component
// Example:
// <Resource
//   name="customers"
//   list={ListGuesser}
//   edit={EditGuesser}
//   show={ShowGuesser}
//   create={CreateGuesser}
// />

export const App = () => (
  <Admin dataProvider={dataProvider} layout={Layout}>
    {/* GENERATED RESOURCES */}
    <Resource
      name="customers"
      list={() => (
        <ListGuesser
          filters={[
            <TextInput label="Search" source="q" alwaysOn key="search" />,
          ]}
        />
      )}
      edit={EditGuesser}
      show={ShowGuesser}
      create={CreateGuesser}
    />
    {/* END GENERATED RESOURCES */}
  </Admin>
);
